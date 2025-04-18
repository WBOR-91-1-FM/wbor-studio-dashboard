use std::{
	sync::Arc,
	hash::Hash,
	cmp::PartialEq,
	marker::PhantomData,
	collections::HashMap
};

use futures::stream::{FuturesUnordered, StreamExt};

use crate::{
	utility_types::{
		vec2f::Vec2f,
		generic_result::*,
		update_rate::UpdateRate,
		time::ReferenceTimestamp,
		dynamic_optional::DynamicOptional
	},

	window_tree::{WindowBorderInfo, WindowContents, WindowUpdaterParams, Window},

	texture::{
		subpool_manager::TextureSubpoolManager,
		pool::{TextureHandle, TexturePool, TextureCreationInfo, RemakeTransitionInfo}
	}
};

////////// This is basically like a cache for image/other data associated with items returned from API calls (expensive to recompute).

/*
TODO:
- Test this with Spinitron at first
- In the end, maybe make an actual unit test for this code or something!
- Eventually, test this code with a really big number of spins (like 200), to see how performance fares with that
- Eake sure that every fn is used here
- Perhaps remove `self` from some of the fns below later on, along with other params, possibly?
- Remove the commented-out print stmts (or neaten them up)
- Could using a `VecDeque` somewhere improve performance? Maybe do some real benchmarking to see what I can do...
*/

/* Note: the `NotNative` type is one which has not been made to a type indended
to represent an object long-term (e.g. JSON, compared to a proper struct).
The `Native` one is one which is has been evaluated to a proper type. */
pub trait ApiHistoryListTraits<Key, NonNative, Native> {
	fn may_need_to_sort_api_results(&self) -> bool;

	fn get_key_from_non_native_item(&self, item: &NonNative) -> Key;
	fn get_timestamp_from_non_native_item(&self, item: &NonNative) -> ReferenceTimestamp;

	fn native_item_is_expired(&self, item: &Native) -> bool;
	fn non_native_item_is_expired(&self, item: &NonNative) -> bool;
	fn action_when_expired(&self, item: &Native); // TODO: perhaps remove, probably!

	fn create_new_local(&self, offshore: &NonNative) -> Arc<Native>;

	// This returns `true` if it just updated. If so, the texture creation info should be updated too.
	fn action_when_updating_local(&self, local: &mut Arc<Native>, offshore: &NonNative) -> bool;

	// I can't use just bytes when dealing with Twilio! So I have to do this (rather than storing `Vec<u8>`s.)
	async fn get_texture_creation_info(&self, item: Arc<Native>) -> Arc<TextureCreationInfo<'static>>;
}

//////////

#[derive(Clone)]
struct ApiHistoryListEntry<Native> {
	just_updated: bool,
	index: usize,
	item: Arc<Native>, // Via the `Arc`, avoid copying of items whenever necessary (given that this would be used in a `ContinuallyUpdated`)
	texture_creation_info: Arc<TextureCreationInfo<'static>> // Same here (`TextureCreationInfo::RawBytes` may be very expensive to copy)
}

#[derive(Clone)]
pub struct APIHistoryList<Key, NonNative, Native, Implementer: ApiHistoryListTraits<Key, NonNative, Native>> {
	max_items: usize,
	keys_to_entries: HashMap<Key, ApiHistoryListEntry<Native>>,
	implementer: Implementer,
	marker: PhantomData<NonNative>
}

impl<Key: PartialEq + Eq + Hash + Copy,
	NonNative,
	Native: Clone,
	Implementer: ApiHistoryListTraits<Key, NonNative, Native>>

	APIHistoryList<Key, NonNative, Native, Implementer> {

	pub fn new(max_items: usize, implementer: Implementer) -> Self {
		APIHistoryList {
			max_items,
			keys_to_entries: HashMap::new(),
			implementer,
			marker: PhantomData
		}
	}

	pub fn get_max_items(&self) -> usize {
		self.max_items
	}

	pub fn get_implementer(&self) -> &Implementer {
		&self.implementer
	}

	pub fn get_implementer_mut(&mut self) -> &mut Implementer {
		&mut self.implementer
	}

	pub async fn update(&mut self, api_results: &mut [NonNative]) -> MaybeError {
		//////////

		// This loops over the API results, and filters out the expired ones
		fn iter_api_results<'a, Key, NonNative, Native>
			(implementer: &'a impl ApiHistoryListTraits<Key, NonNative, Native>, api_results: &'a [NonNative])
			-> impl Iterator<Item = (usize, &'a NonNative)> {

			api_results.iter().filter(|api_result| !implementer.non_native_item_is_expired(api_result)).enumerate()
		}

		//////////

		/*
		if api_results.len() < self.max_items {
			log::warn!("Hm, some API results may be missing...");
		}
		*/

		if self.implementer.may_need_to_sort_api_results() {
			// 1. Sort the API results (TODO: use insertion sort instead, since the data will likely be mostly sorted)
			api_results.sort_by(|r1, r2| {
				let t1 = self.implementer.get_timestamp_from_non_native_item(r1);
				let t2 = self.implementer.get_timestamp_from_non_native_item(r2);

				// TODO: do more elaborate sorting when doing it for Twilio (make a sorting tiebreaker function)
				t2.cmp(&t1) // TODO: do the opposite order for Twilio (and allow specifying the opposite sorting order)
			});
		}

		//////////

		let mut texture_creation_info_update_set = FuturesUnordered::new();

		/* Retaining locals that are not expired, and are also in the offshore
		(otherwise, removing them from the map, and calling `action_when_expired`) */
		self.keys_to_entries.retain(|local_key, local_entry| {
			local_entry.just_updated = false;

			// 2. Expire the local value if needed
			if self.implementer.native_item_is_expired(&local_entry.item) {
				// println!("Expiry case #1"); // TODO: see that this happens with Twilio
				self.implementer.action_when_expired(&local_entry.item);
				return false;
			}

			// Checking if the local exists in the offshore
			for (api_result_index, api_result) in iter_api_results(&self.implementer, api_results) {

				// 3. Retain locals that are in the offshore (and update them as well)
				if &self.implementer.get_key_from_non_native_item(api_result) == local_key {
					local_entry.index = api_result_index; // Update the index, in case things shifted
					local_entry.just_updated = self.implementer.action_when_updating_local(&mut local_entry.item, api_result);

					// TODO: perhaps put an `Arc` around the key?
					let cloned_key = *local_key;
					let texture_creation_info = self.implementer.get_texture_creation_info(local_entry.item.clone());

					texture_creation_info_update_set.push(async move {
						(cloned_key, texture_creation_info.await)
					});

					return true;
				}
			}

			// println!("Expiry case #2 for spin");

			// Remove locals that are not in the offshore
			self.implementer.action_when_expired(&local_entry.item);
			false
		});

		// Updating the texture creation info for just-updated entries (TODO: can I do this in a more elegant way?)
		while let Some((key, texture_creation_info)) = texture_creation_info_update_set.next().await {
			self.keys_to_entries.get_mut(&key).unwrap().texture_creation_info = texture_creation_info;
		}

		////////// 4. Add new locals from new offshore values

		let mut new_entry_set = FuturesUnordered::new();

		for (api_history_index, api_result) in iter_api_results(&self.implementer, api_results) {
			let key = self.implementer.get_key_from_non_native_item(api_result);

			if !self.keys_to_entries.contains_key(&key) {
				// println!("(plain history list) Insert new local with key");
				let local = self.implementer.create_new_local(api_result);
				let result = self.implementer.get_texture_creation_info(local.clone());

				new_entry_set.push(async move {
					(key, (local, result.await, api_history_index))
				});
			}
		}

		while let Some((key, (local, texture_creation_info, api_history_index))) = new_entry_set.next().await {
			self.keys_to_entries.insert(key, ApiHistoryListEntry {
				just_updated: false,
				index: api_history_index,
				item: local,
				texture_creation_info
			});
		}

		Ok(())
	}
}

//////////

pub struct ApiHistoryListTextureManager<Key, NonNative, Native, Implementer> {
	max_items: usize,

	keys_to_textures: HashMap<Key, TextureHandle>,

	texture_subpool_manager: TextureSubpoolManager,
	maybe_remake_transition_info: Option<RemakeTransitionInfo>,

	marker: PhantomData<(Native, NonNative, Implementer)>
}

// TODO: with the new `Vec`, there's 3 hash tables and 1 list in play (can I reduce this count)?
impl<Key: PartialEq + Eq + Hash + Clone, NonNative, Native,
	Implementer: ApiHistoryListTraits<Key, NonNative, Native>>
	ApiHistoryListTextureManager<Key, NonNative, Native, Implementer> {

	pub fn new(max_items: usize, maybe_remake_transition_info: Option<RemakeTransitionInfo>) -> Self {
		ApiHistoryListTextureManager {
			max_items,
			keys_to_textures: HashMap::new(),
			texture_subpool_manager: TextureSubpoolManager::new(max_items),
			maybe_remake_transition_info,
			marker: PhantomData
		}
	}

	pub fn update_from_history_list(&mut self,
		api_history_list: &APIHistoryList<Key, NonNative, Native, Implementer>,
		texture_pool: &mut TexturePool) -> MaybeError {

		////////// 1. Retain unexpired locals that are also in the offshore

		self.keys_to_textures.retain(|local_tex_key, local_tex_value| {
			if let Some(entry) = api_history_list.keys_to_entries.get(local_tex_key) {
				if entry.just_updated {
					// println!("(history list tex manager) >>> Action when updating local");

					if let Err(err) = self.texture_subpool_manager.re_request_slot(
						local_tex_value,
						&entry.texture_creation_info,
						self.maybe_remake_transition_info.as_ref(),
						texture_pool
					) {
						// TODO: handle this better
						log::error!("Error when re-requesting a slot: '{err}'");
					}
				}

				true
			}
			else {
				// println!("(history list tex manager) >>> Action when expired (#2)");
				self.texture_subpool_manager.give_back_slot(local_tex_value);
				false
			}
		});

		////////// 2. Add new locals from offshore values

		for (offshore_key, entry) in &api_history_list.keys_to_entries {
			if !self.keys_to_textures.contains_key(offshore_key) {
				let local = self.texture_subpool_manager.request_slot(
					&entry.texture_creation_info,
					self.maybe_remake_transition_info.as_ref(),
					texture_pool
				)?;

				// println!("(history list tex manager) >>> Create new local");
				self.keys_to_textures.insert(offshore_key.clone(), local.clone());
			}
		}

		Ok(())

	}

	pub fn get_texture_at_index(&self, index: usize,
		api_history_list: &APIHistoryList<Key, NonNative, Native, Implementer>) -> Option<TextureHandle> {

		assert!(api_history_list.max_items == self.max_items);

		/*
		if index >= api_history_list.keys_to_entries.len() {
			println!("No texture exists yet here!");
		}
		*/

		for (k, v) in &api_history_list.keys_to_entries {
			if index == v.index { // TODO: fix the time complexity here
				return self.keys_to_textures.get(k).cloned();
			}
		}

		None
	}
}

//////////

pub fn make_api_history_list_window(
	overall_tl: Vec2f, overall_size: Vec2f, subwindow_size: Vec2f,
	border_info: WindowBorderInfo,
	item_updater_fns: &[(fn(WindowUpdaterParams) -> MaybeError, UpdateRate)],
	subwindow_info: impl Iterator<Item = (Vec2f, WindowBorderInfo)> // Top left, and border info
) -> Window {

	let subwindows = subwindow_info.enumerate().map(|(i, (tl, border_info))|
		Window::new(
			item_updater_fns.to_vec(),
			DynamicOptional::new(i),
			WindowContents::Nothing,
			border_info,
			tl,
			subwindow_size,
			vec![]
		)
	).collect();

	Window::new(
		vec![],
		DynamicOptional::NONE,
		WindowContents::Nothing,
		border_info,
		overall_tl,
		overall_size,
		subwindows
	)
}
