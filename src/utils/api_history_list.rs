use std::{
	hash::Hash,
	collections::HashMap
};

use futures::stream::{FuturesUnordered, StreamExt};

use crate::{
	utils::{
		vec2f::Vec2f,
		generic_result::*,
		update_rate::UpdateRate,
		dynamic_optional::DynamicOptional
	},

	window_tree::{WindowBorderInfo, WindowContents, WindowUpdaterParams, Window},
	texture::pool::{TextureHandle, TexturePool, TextureCreationInfo, RemakeTransitionInfo}
};

////////// This is used for managing a subset of textures used in the texture pool

// TODO: could I keep 2 piles instead (one for unused, and one for used)?
pub struct TextureSubpoolManager {
	subpool: HashMap<TextureHandle, bool>, // The boolean is true if it's used, otherwise unused
	max_size: usize // TODO: can I avoid keeping this here?
}

impl TextureSubpoolManager {
	pub fn new(subpool_size: usize) -> Self {
		Self {subpool: HashMap::with_capacity(subpool_size), max_size: subpool_size}
	}

	pub fn request_slot(&mut self, texture_creation_info: &TextureCreationInfo,
		maybe_remake_transition_info: Option<&RemakeTransitionInfo>,
		texture_pool: &mut TexturePool) -> GenericResult<TextureHandle> {

		assert!(self.subpool.len() <= self.max_size);

		// If this is the case, go and check for unused variants
		if self.subpool.len() == self.max_size {
			for (texture, is_used) in &mut self.subpool {
				if !*is_used {
					// println!("(request) doing re-request, and setting {texture:?} to used");
					*is_used = true;
					texture_pool.remake_texture(texture_creation_info, texture, maybe_remake_transition_info)?;
					return Ok(texture.clone());
				}
			}

			panic!("No textures available for requesting in subpool!");
		}
		else {
			let texture = texture_pool.make_texture(texture_creation_info)?;

			if self.subpool.insert(texture.clone(), true).is_some() {
				panic!("This texture was already allocated in the subpool!");
			}

			// println!("(request) setting {texture:?} to used");

			Ok(texture)
		}
	}

	pub fn re_request_slot(&mut self,
		incoming_texture: &TextureHandle,
		texture_creation_info: &TextureCreationInfo,
		maybe_remake_transition_info: Option<&RemakeTransitionInfo>,
		texture_pool: &mut TexturePool) -> MaybeError {

		if let Some(is_used) = self.subpool.get(incoming_texture) {
			// println!("(re-request) checking {incoming_texture:?} for being used before");
			assert!(is_used);
			// println!("(re-request) doing re-request for {incoming_texture:?}");
			texture_pool.remake_texture(texture_creation_info, incoming_texture, maybe_remake_transition_info)
		}
		else {
			panic!("Slot was not previously allocated in subpool!");
		}
	}

	// TODO: would making the incoming texture `mut` stop further usage of it?
	pub fn give_back_slot(&mut self, incoming_texture: &TextureHandle) {
		if let Some(is_used) = self.subpool.get_mut(incoming_texture) {
			// println!("(give back) checking {incoming_texture:?} for being used before");
			assert!(*is_used);
			// println!("(give back) setting {incoming_texture:?} to unused");
			*is_used = false;
		}
		else {
			panic!("Incoming texture did not already exist in subpool!");
		}
	}
}

////////// `ApiHistoryList` is basically like a cache for image/other data associated with items returned from API calls (expensive to recompute).

/*
TODO:
- In the end, maybe make an actual unit test for this code or something!
- Eventually, test this code with a really big number of spins (like 200), to see how performance fares with that
- Could using a `VecDeque` somewhere improve performance? Maybe do some real benchmarking to see what I can do...
*/

/* Note: the `NotNative` type is one which has not been made to a type indended
to represent an object long-term (e.g. JSON, compared to a proper struct).
The `Native` one is one which is has been evaluated to a proper type. */
pub trait ApiHistoryListImplementer {
	type Key: Eq + Hash + Clone;

	type NonNative;
	type Native;

	type Param;
	type ResolveTextureCreationInfoParam;
	type IntermediateTextureCreationInfo;

	fn may_need_to_sort_api_results() -> bool;
	fn compare(a: &Self::NonNative, b: &Self::NonNative) -> std::cmp::Ordering;

	fn get_key(offshore: &Self::NonNative) -> Self::Key;
	fn is_expired(param: &Self::Param, offshore: &Self::NonNative) -> bool;

	fn create_new_local(param: &Self::Param, offshore: &Self::NonNative) -> Self::Native;

	/*  This returns `true` if it just updated. If so, the texture creation info will be updated too.
	Note: `offshore` could be added as param later on if needed (i.e. if the offshore value is expected to change). */
	fn update_local(param: &Self::Param, local: &mut Self::Native) -> bool;

	async fn get_intermediate_texture_creation_info(param: &Self::Param, local: &Self::Native) -> Self::IntermediateTextureCreationInfo;

	fn resolve_texture_creation_info<'a>(param: &Self::ResolveTextureCreationInfoParam,
		local: &Self::Native, intermediate_texture_creation_info: &'a Self::IntermediateTextureCreationInfo) -> TextureCreationInfo<'a>;
}

//////////

#[derive(Clone)]
struct ApiHistoryListEntry<Native, IntermediateTextureCreationInfo> {
	just_updated: bool,
	index: usize,
	item: Native, // Clients can use `Arc` over `Native` if they want to; it depends on how heavy the type is
	intermediate_texture_creation_info: IntermediateTextureCreationInfo
}

#[derive(Clone)]
pub struct ApiHistoryList<Implementer: ApiHistoryListImplementer> {
	max_items: usize,
	keys_to_entries: HashMap<Implementer::Key, ApiHistoryListEntry<Implementer::Native, Implementer::IntermediateTextureCreationInfo>>
}

impl<Implementer: ApiHistoryListImplementer> ApiHistoryList<Implementer> {
	pub fn new(max_items: usize) -> Self {
		ApiHistoryList {
			max_items,
			keys_to_entries: HashMap::with_capacity(max_items)
		}
	}

	pub fn get_max_items(&self) -> usize {
		self.max_items
	}

	// This loops over the API results, and filters out the expired ones
	fn iter_api_results<'a>(param: &'a Implementer::Param,
		api_results: &'a [Implementer::NonNative]) -> impl Iterator<Item = (usize, &'a Implementer::NonNative)> {

		api_results.iter().filter(
			|api_result| !Implementer::is_expired(param, api_result)
		).enumerate()
	}

	// TODO: perhaps enforce the max items with the API results here?
	pub async fn update(&mut self, api_results: &mut [Implementer::NonNative], param: Implementer::Param) {
		// TODO: maybe let the caller handle the sorting?
		if Implementer::may_need_to_sort_api_results() {
			/* 1. Sort the API results (TODO: use insertion sort instead, since the data will likely be mostly sorted.
			And once that's done, ensure that the whole list isn't reversed; since in that case, insertion sort will still be quite expensive.
			Just reverse the indexing direction in that case. Also ensure that it still looks right when stuff is expired on-screen.) */
			api_results.sort_by(|r1, r2| Implementer::compare(r1, r2));
		}

		////////// 2. Retain locals that are in the offshore (and update them as well)

		// TODO: avoid the time complexity of the initial `retain` loop (it's `O(n^2))`
		let mut any_updated = false;

		self.keys_to_entries.retain(|local_key, local_entry| {
			local_entry.just_updated = false;

			// Checking if the local exists in the offshore
			for (api_result_index, api_result) in Self::iter_api_results(&param, api_results) {
				if &Implementer::get_key(api_result) == local_key {
					local_entry.index = api_result_index; // Update the index, in case things shifted (TODO: can I use a shifting index as a sign to do a remake transition somehow, maybe?)
					local_entry.just_updated = Implementer::update_local(&param, &mut local_entry.item);

					if local_entry.just_updated {
						any_updated = true;
					}

					return true;
				}
			}

			false // Remove locals that are not in the offshore (prefiltered for expiry already)
		});

		////////// 3. Update the textures of just-updated locals (TODO: can I move this code into the loop above? Or is that impossible?)

		if any_updated {
			let mut texture_update_set = FuturesUnordered::new();

			for local_entry in self.keys_to_entries.values_mut() {
				if local_entry.just_updated {
					texture_update_set.push(async {
						local_entry.intermediate_texture_creation_info = Implementer::get_intermediate_texture_creation_info(&param, &local_entry.item).await;
					});
				}
			}

			// TODO: is it theoretically possible to run all of the texture-updating futures concurrently?
			while texture_update_set.next().await.is_some() {}
		}

		////////// 4. Add new locals from new offshore values

		let mut new_entry_set = FuturesUnordered::new();

		for (api_history_index, api_result) in Self::iter_api_results(&param, api_results) {
			let key = Implementer::get_key(api_result);

			if !self.keys_to_entries.contains_key(&key) {
				let index = async move {api_history_index}; // TODO: how to avoid this stupid index-move situation? I can't access it from the closure otherwise?
				let local = Implementer::create_new_local(&param, api_result);

				new_entry_set.push(async {
					let intermediate = Implementer::get_intermediate_texture_creation_info(&param, &local).await;
					(key, index.await, local, intermediate)
				});
			}
		}

		while let Some((key, api_history_index, local, intermediate)) = new_entry_set.next().await {
			self.keys_to_entries.insert(key, ApiHistoryListEntry {
				just_updated: false,
				index: api_history_index,
				item: local,
				intermediate_texture_creation_info: intermediate
			});
		}
	}
}

//////////

/* TODO: in total, I'm using 3 hash tables at the moment. Can I reduce this count? Maybe inline `TextureSubpoolManager` into
`keys_to_textures` somehow?  And consider using textures as keys themselves somehow? It's done for `TextureSubPoolManager` already...
Also, there are three key types to consider: an index, the implementer key, or texture handles as a key. */
pub struct ApiHistoryListTextureManager<Implementer: ApiHistoryListImplementer> {
	max_items: usize,
	keys_to_textures: HashMap<Implementer::Key, TextureHandle>,
	texture_subpool_manager: TextureSubpoolManager,
	maybe_remake_transition_info: Option<RemakeTransitionInfo>
}

impl<Implementer: ApiHistoryListImplementer> ApiHistoryListTextureManager<Implementer> {
	pub fn new(max_items: usize, maybe_remake_transition_info: Option<RemakeTransitionInfo>) -> Self {
		ApiHistoryListTextureManager {
			max_items,
			keys_to_textures: HashMap::with_capacity(max_items),
			texture_subpool_manager: TextureSubpoolManager::new(max_items),
			maybe_remake_transition_info
		}
	}

	pub fn update_from_history_list(&mut self,
		api_history_list: &ApiHistoryList<Implementer>,
		texture_pool: &mut TexturePool, param: &Implementer::ResolveTextureCreationInfoParam) {

		////////// 1. Retain unexpired locals that are also in the offshore

		self.keys_to_textures.retain(|local_tex_key, local_tex_value| {
			if let Some(entry) = api_history_list.keys_to_entries.get(local_tex_key) {
				if entry.just_updated {
					let as_texture_creation_info = Implementer::resolve_texture_creation_info(
						param, &entry.item, &entry.intermediate_texture_creation_info
					);

					let maybe_re_request_error = self.texture_subpool_manager.re_request_slot(
						local_tex_value, &as_texture_creation_info,
						self.maybe_remake_transition_info.as_ref(), texture_pool
					);

					if let Err(err) = maybe_re_request_error {
						log::error!("Error when re-requesting a slot: '{err}'"); // TODO: handle this better
					}
				}

				true
			}
			else {
				self.texture_subpool_manager.give_back_slot(local_tex_value);
				false
			}
		});

		////////// 2. Add new locals from offshore values

		for (offshore_key, entry) in &api_history_list.keys_to_entries {
			if !self.keys_to_textures.contains_key(offshore_key) {

				let as_texture_creation_info = Implementer::resolve_texture_creation_info(
					param, &entry.item, &entry.intermediate_texture_creation_info
				);

				match self.texture_subpool_manager.request_slot(&as_texture_creation_info, self.maybe_remake_transition_info.as_ref(), texture_pool) {
					Ok(local) => {
						self.keys_to_textures.insert(offshore_key.clone(), local);
					}
					Err(err) => {
						log::error!("Error when requesting a slot: '{err}'");
						continue;
					}
				}

			}
		}
	}

	pub fn get_texture_at_index(&self, index: usize, api_history_list: &ApiHistoryList<Implementer>) -> Option<TextureHandle> {
		assert!(api_history_list.max_items == self.max_items);

		// TODO: fix the time complexity here
		for (key, entry) in &api_history_list.keys_to_entries {
			if index == entry.index {
				return self.keys_to_textures.get(key).cloned();
			}
		}

		None
	}
}

//////////

pub struct ApiHistoryListSubWindowInfo {
	pub top_left: Vec2f,
	pub main_window_zoom_factor: Vec2f,

	pub background_contents: WindowContents,
	pub skip_aspect_ratio_correction_for_background_contents: bool
}

type ItemUpdaterFn = fn(WindowUpdaterParams) -> MaybeError;

pub fn make_api_history_list_window(
	contained_area: (Vec2f, Vec2f),
	contained_area_border_info: WindowBorderInfo,
	subwindow_size: Vec2f,
	item_updater_fns: &[(ItemUpdaterFn, UpdateRate)],
	subwindow_info: impl Iterator<Item = ApiHistoryListSubWindowInfo>
) -> Window {

	let children = subwindow_info.enumerate().map(|(i, subwindow_info)| {
		let zoom_factor = subwindow_info.main_window_zoom_factor;

		let window = Window::new(
			item_updater_fns.to_vec(),
			DynamicOptional::new(i),
			WindowContents::Nothing,
			None,

			zoom_factor * Vec2f::new_scalar(0.5),
			Vec2f::ONE - zoom_factor,

			vec![]
		);

		let mut parent_with_contents = Window::new(
			vec![],
			DynamicOptional::NONE,
			subwindow_info.background_contents,
			None,

			subwindow_info.top_left,
			subwindow_size,

			vec![window]
		);

		let skip = subwindow_info.skip_aspect_ratio_correction_for_background_contents;
		parent_with_contents.set_aspect_ratio_correction_skipping(skip);

		parent_with_contents
	}).collect();

	Window::new(
		vec![],
		DynamicOptional::NONE,
		WindowContents::Nothing,
		contained_area_border_info,
		contained_area.0,
		contained_area.1,
		children
	)
}
