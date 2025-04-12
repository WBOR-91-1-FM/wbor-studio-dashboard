use std::{
	sync::Arc,
	borrow::Cow
};

use futures::{
	StreamExt,
	stream::FuturesUnordered
};

use crate::{
	request,
	dashboard_defs::error::ErrorState,
	texture::pool::{TexturePool, TextureHandle, TextureCreationInfo},

	utility_types::{
		time::*,
		file_utils,
		hash::hash_obj,
		generic_result::*,
		continually_updated::{ContinuallyUpdatable, ContinuallyUpdated, ContinuallyUpdatedState},
		api_history_list::{APIHistoryList, ApiHistoryListTraits, ApiHistoryListTextureManager}
	},

	spinitron::{
		wrapper_types::SpinitronModelId,

		model::{
			MaybeTextureCreationInfo,
			NUM_SPINITRON_MODEL_TYPES,
			Spin, Playlist, Persona, Show,
			SpinitronModel, SpinitronModelName
		}
	}
};

////////// Model age stuff:

#[derive(Clone, PartialEq)]
pub enum ModelAgeState {
	BeforeIt, // TODO: can this state ever even happen? Test for this...
	CurrentlyActive,
	AfterIt,
	AfterItFromCustomExpiryDuration
}

#[derive(Clone)]
struct ModelAgeData {
	custom_expiry_duration: Duration,
	curr_age_state: ModelAgeState,
	just_updated_state: bool
}

impl ModelAgeData {
	fn new(custom_expiry_duration: Duration, model: &dyn SpinitronModel) -> GenericResult<Self> {
		let data = Self {
			custom_expiry_duration,
			curr_age_state: ModelAgeState::CurrentlyActive,
			just_updated_state: false
		};

		data.update(model)
	}

	// This returns the new model age data
	fn update(mut self, model: &dyn SpinitronModel) -> GenericResult<ModelAgeData> {
		if let Some((start_time, end_time)) = model.maybe_get_time_range()? {
			let curr_time = get_reference_time();

			let time_after_start = curr_time.signed_duration_since(start_time);
			let time_after_end = curr_time.signed_duration_since(end_time);

			const ZERO: Duration = Duration::zero();

			// The custom end may be before or after the actual end
			let (is_after_start, is_after_end, is_after_custom_end) = (
				time_after_start > ZERO,
				time_after_end > ZERO,
				time_after_end > self.custom_expiry_duration
			);

			let new_age_state = if is_after_start {
				if is_after_custom_end {
					/* The first branch is for when the custom expiry is before the
					actual end time; so then, give priority to the actual end */
					if is_after_end && self.custom_expiry_duration < ZERO {ModelAgeState::AfterIt}
					else {ModelAgeState::AfterItFromCustomExpiryDuration}
				}
				else if is_after_end {ModelAgeState::AfterIt}
				else {ModelAgeState::CurrentlyActive}
			}
			else {
				ModelAgeState::BeforeIt
			};

			self.just_updated_state = self.curr_age_state != new_age_state;
			self.curr_age_state = new_age_state;
		}

		Ok(self)
	}
}

////////// The implementer for the spin history list trait:

type WindowSize = (u32, u32);

#[derive(Clone)]
struct SpinHistoryListTraitImplementer {
	get_fallback_texture_creation_info: fn() -> TextureCreationInfo<'static>,
	item_texture_size: WindowSize,
	just_found_true_texture_size: bool
}

impl ApiHistoryListTraits<SpinitronModelId, Spin, Spin> for SpinHistoryListTraitImplementer {
	fn may_need_to_sort_api_results(&self) -> bool {
		false // Spins come in order!
	}

	fn get_key_from_non_native_item(&self, offshore: &Spin) -> SpinitronModelId {
		offshore.get_id()
	}

	fn get_timestamp_from_non_native_item(&self, offshore: &Spin) -> ReferenceTimestamp {
		offshore.get_start_time()
	}

	fn native_item_is_expired(&self, _: &Spin) -> bool {
		false
	}

	fn non_native_item_is_expired(&self, _offshore: &Spin) -> bool {
		false
	}

	fn action_when_expired(&self, _: &Spin) {

	}

	fn create_new_local(&self, offshore: &Spin) -> Arc<Spin> {
		Arc::new(offshore.clone())
	}

	fn action_when_updating_local(&self, mut _local: &mut Arc<Spin>, _offshore: &Spin) -> bool {
		// Only updating local if the true texture size was just found
		self.just_found_true_texture_size
	}

	async fn get_texture_creation_info(&self, spin: Arc<Spin>) -> Arc<TextureCreationInfo<'static>> {
		let maybe_info = spin.get_texture_creation_info(ModelAgeState::CurrentlyActive, self.item_texture_size);

		let bytes = get_model_texture_bytes(
			maybe_info, self.get_fallback_texture_creation_info
		).await.unwrap(); // This expects that the fallback texture will work (otherwise, we have a serious issue!)

		Arc::new(TextureCreationInfo::RawBytes(Cow::Owned(bytes)))
	}
}

////////// Defining some types pertaining to `SpinitronStateData`

#[derive(Clone, Default)]
struct ModelDataCacheEntry {
	texture_bytes: Arc<Vec<u8>>, // This is an `Arc` to avoid the cost of copying
	texture_creation_info_hash: u64,
	texture_creation_info_hash_changed: bool,

	string: Arc<String>,
	string_changed: bool
}

#[derive(Clone)]
struct SpinitronStateData {
	api_key: Arc<String>,

	spin: Arc<Spin>,
	playlist: Arc<Playlist>,
	persona: Arc<Persona>,
	show: Arc<Show>,

	// TODO: perhaps merge these two
	age_data: [ModelAgeData; NUM_SPINITRON_MODEL_TYPES],
	cached_model_data: [ModelDataCacheEntry; NUM_SPINITRON_MODEL_TYPES],

	spin_history_list: APIHistoryList<SpinitronModelId, Spin, Spin, SpinHistoryListTraitImplementer>
}

// The third param is the fallback texture creation info, and the fourth one is the spin window size
type SpinitronStateDataParams<'a> = (
	&'a str, // API key
	fn() -> TextureCreationInfo<'static>, // Fallback texture creation info getter
	Duration, // The API update rate
	[Duration; NUM_SPINITRON_MODEL_TYPES], // Custom model expiry durations
	WindowSize, // The spin texture size (for the primary spin)
	WindowSize, // The spin history item texture size
	usize // The number of spins shown in the history
);

//////////

async fn get_model_texture_bytes(
	texture_creation_info: MaybeTextureCreationInfo<'_>,
	get_fallback_texture_creation_info: fn() -> TextureCreationInfo<'static>) -> GenericResult<Vec<u8>> {

	async fn load_texture_creation_info_bytes(info: &TextureCreationInfo<'_>) -> GenericResult<Vec<u8>> {
		/* I am doing this to speed up the loading of textures on the main
		thread, by doing the image URL requesting on this task/thread instead,
		and precaching anything from disk in byte form as well. */
		match info {
			TextureCreationInfo::Path(path) =>
				file_utils::read_file_contents(path).await,

			TextureCreationInfo::Url(url) => {
				let response = request::get(url).await?;
				let bytes = response.bytes().await?;
				Ok(bytes.to_vec())
			}

			TextureCreationInfo::RawBytes(_) =>
				panic!("Spinitron model textures should not be returning raw bytes!"),

			TextureCreationInfo::Text(_) =>
				panic!("Precaching the text texture creation info is not supported for plain Spinitron model textures!")
		}
	}

	let info = texture_creation_info.unwrap_or_else(get_fallback_texture_creation_info);

	match load_texture_creation_info_bytes(&info).await {
		Err(err) => {
			log::warn!("Reverting to fallback texture for Spinitron model. Error: '{err}'");
			load_texture_creation_info_bytes(&get_fallback_texture_creation_info()).await
		},

		info => info
	}
}

//////////

impl SpinitronStateData {
	fn new((api_key, get_fallback_texture_creation_info, _,
		custom_model_expiry_durations, _, spin_history_item_texture_size,
		num_spins_shown_in_history): SpinitronStateDataParams) -> Self {

		//////////

		fn arc_default<T: Default>() -> Arc<T> {
			Arc::new(T::default())
		}

		let (spin, playlist, persona, show) = (
			arc_default::<Spin>(), arc_default::<Playlist>(), arc_default::<Persona>(), arc_default::<Show>()
		);

		let models_with_custom_expiry_durations: [(&dyn SpinitronModel, Duration); NUM_SPINITRON_MODEL_TYPES] = [
			(spin.as_ref(), custom_model_expiry_durations[0]),
			(playlist.as_ref(), custom_model_expiry_durations[1]),
			(persona.as_ref(), custom_model_expiry_durations[2]),
			(show.as_ref(), custom_model_expiry_durations[3])
		];

		// TODO: don't unwrap once `try_map` becomes stable
		let age_data = models_with_custom_expiry_durations
			.map(|(model, custom_expiry_duration)|
				ModelAgeData::new(custom_expiry_duration, model).unwrap()
			);

		//////////

		Self {
			api_key: Arc::new(api_key.to_owned()),

			spin, playlist, persona, show,

			age_data,
			cached_model_data: std::array::from_fn(|_| ModelDataCacheEntry::default()),

			spin_history_list: APIHistoryList::new(
				num_spins_shown_in_history,

				SpinHistoryListTraitImplementer {
					get_fallback_texture_creation_info,
					item_texture_size: spin_history_item_texture_size,
					just_found_true_texture_size: false
				}
			)
		}
	}

	async fn compute_cacheable_model_data(&self, model_name: SpinitronModelName, spin_texture_size: WindowSize) -> ModelDataCacheEntry {
		let model = self.get_model_by_name(model_name);
		let age_state = self.age_data[model_name as usize].curr_age_state.clone();

		let texture_creation_info = model.get_texture_creation_info(age_state, spin_texture_size);
		let texture_creation_info_hash = hash_obj(&texture_creation_info);

		let prev_entry = &self.cached_model_data[model_name as usize];
		let age_state = self.age_data[model_name as usize].curr_age_state.clone();

		let maybe_new_model_string = model.to_string(age_state);

		////////// Doing this hashing stuff, because we don't want to invoke a transition when a model's ID changes, and either of its textures stays the same

		let texture_creation_info_hash_changed = texture_creation_info_hash != prev_entry.texture_creation_info_hash;
		let string_changed = maybe_new_model_string != prev_entry.string.as_str();

		let texture_bytes = if texture_creation_info_hash_changed {
			let get_fallback_texture_creation_info =
				self.spin_history_list.get_implementer().get_fallback_texture_creation_info;

			Arc::new(
				get_model_texture_bytes(texture_creation_info, get_fallback_texture_creation_info)
				.await.unwrap()
			)
		}
		else {
			prev_entry.texture_bytes.clone()
		};

		let string = if string_changed {
			Arc::new(maybe_new_model_string.into_owned())
		}
		else {
			prev_entry.string.clone()
		};

		//////////

		ModelDataCacheEntry {
			texture_bytes,
			texture_creation_info_hash,
			texture_creation_info_hash_changed,

			string,
			string_changed
		}
	}

	fn get_models(&self) ->  [&dyn SpinitronModel; NUM_SPINITRON_MODEL_TYPES] {
		[self.spin.as_ref(), self.playlist.as_ref(), self.persona.as_ref(), self.show.as_ref()]
	}

	const fn get_model_names() -> &'static [SpinitronModelName; NUM_SPINITRON_MODEL_TYPES] {
		const MODEL_NAMES: [SpinitronModelName; NUM_SPINITRON_MODEL_TYPES] = [
			SpinitronModelName::Spin, SpinitronModelName::Playlist, SpinitronModelName::Persona, SpinitronModelName::Show
		];

		&MODEL_NAMES
	}

	pub fn get_model_by_name(&self, model_name: SpinitronModelName) -> &dyn SpinitronModel {
		match model_name {
			SpinitronModelName::Spin => self.spin.as_ref(),
			SpinitronModelName::Playlist => self.playlist.as_ref(),
			SpinitronModelName::Persona => self.persona.as_ref(),
			SpinitronModelName::Show => self.show.as_ref()
		}
	}

	async fn sync_models(&mut self) -> MaybeError {
		let api_key = self.api_key.as_str();

		//////////

		let spin_future = async {
			// Step 1: get the current spin, and the spin history.
			let (maybe_new_spin, mut spin_history) = Spin::get_current_and_history(
				api_key, self.spin_history_list.get_max_items()
			).await?;

			if maybe_new_spin.get_id() != self.spin.get_id() {
				self.spin = Arc::new(maybe_new_spin);
			}

			// Step 2: update the spin history list.
			self.spin_history_list.update(&mut spin_history).await?;

			// Explicitly defining the result here is needed for type inference of `Ok(())` in other places
			let result: MaybeError = Ok(());
			result
		};

		let playlist_and_persona_future = async {
			/* Step 3: get a maybe new playlist (don't base it on a spin ID,
			since the spin may not belong to a playlist under automation). */
			let maybe_new_playlist = Playlist::get(api_key).await?;

			if maybe_new_playlist.get_id() != self.playlist.get_id() {
				/* Step 4: get the persona id based on the playlist id (since otherwise, you'll
				just get some persona that's first in Spinitron's internal list of personas. */
				self.persona = Arc::new(Persona::get(api_key, &maybe_new_playlist).await?);
				self.playlist = Arc::new(maybe_new_playlist);
			}

			Ok(())
		};

		let show_future = async {
			let curr_minutes = get_local_time().minute();

			// Shows can only be scheduled under 30-minute intervals (will not switch immediately if added sporadically)
			if curr_minutes == 0 || curr_minutes == 30 {
				/* Step 5: get the current show id (based on what's on the
				schedule, irrespective of what show was last on).
				This is not in the branch above, since the show should
				change directly on schedule, not when a new playlist is made. */
				self.show = Arc::new(Show::get(api_key).await?);
			}

			Ok(())
		};

		//////////

		tokio::try_join!(
			spin_future,
			playlist_and_persona_future,
			show_future
		)?;

		Ok(())
	}
}

impl ContinuallyUpdatable for SpinitronStateData {
	type Param = (WindowSize, WindowSize);

	async fn update(&mut self, (spin_texture_size, spin_history_item_texture_size): &Self::Param) -> MaybeError {
		////////// Update some variables associated with the spin history list

		let implementer = self.spin_history_list.get_implementer_mut();

		implementer.just_found_true_texture_size = implementer.item_texture_size != *spin_history_item_texture_size;

		if implementer.just_found_true_texture_size {
			implementer.item_texture_size = *spin_history_item_texture_size;
		}

		////////// Update the models

		let get_model_ids = |data: &Self|
			data.get_models().map(|model| model.get_id());

		let original_ids = get_model_ids(self);
		self.sync_models().await?;
		let new_ids = get_model_ids(self);

		////////// Collect futures for new models to cache

		let new_to_cache_futures = FuturesUnordered::new();

		// TODO: how to do this without all the indexing?
		for model_name in Self::get_model_names() {
			let i = *model_name as usize;

			// Under these conditions, the texture may have updated (sometimes, models will have the same texture across different IDs though)
			// TODO: perhaps also check based on a texture or text updating? The id may not be definitive... (e.g. changing the album cover after submitting a spin)
			let maybe_updated = original_ids[i] != new_ids[i] || self.age_data[i].just_updated_state;

			if maybe_updated {
				new_to_cache_futures.push(async {
					let deref_model_name = *model_name;
					(deref_model_name as usize, self.compute_cacheable_model_data(deref_model_name, *spin_texture_size).await)
				});
			}
		}

		//////////

		// TODO: can I avoid this `collect` perhaps, somehow?
		let new_to_cache = new_to_cache_futures.collect::<Vec<_>>().await;

		// First, invalidate the cache
		for item in &mut self.cached_model_data {
			item.texture_creation_info_hash_changed = false; // Marking the texture as not updated
			item.string_changed = false; // Marking the text as not updated
		}

		// Then, add the new elements
		for (index, cache_entry) in new_to_cache {
			self.cached_model_data[index] = cache_entry;
		}

		//////////

		Ok(())
	}
}

//////////

pub struct SpinitronState {
	continually_updated: ContinuallyUpdated<SpinitronStateData>,
	just_got_new_continual_data: bool,

	history_list_texture_manager: ApiHistoryListTextureManager<SpinitronModelId, Spin, Spin, SpinHistoryListTraitImplementer>,
	spin_history_texture_size: WindowSize
}

impl SpinitronState {
	pub fn new(params: SpinitronStateDataParams) -> GenericResult<Self> {
		let (.., api_update_rate, _, initial_spin_texture_size_guess,
			initial_spin_history_texture_size_guess, max_spin_history_items
		) = params;

		let data = SpinitronStateData::new(params);
		let texture_size_guesses = (initial_spin_texture_size_guess, initial_spin_history_texture_size_guess);

		let continually_updated = ContinuallyUpdated::new(
			data, texture_size_guesses, "Spinitron", api_update_rate
		);

		Ok(Self {
			continually_updated,
			just_got_new_continual_data: false,

			history_list_texture_manager: ApiHistoryListTextureManager::new(max_spin_history_items, None),
			spin_history_texture_size: initial_spin_history_texture_size_guess
		})
	}

	fn make_correct_texture_size_from_window_size(window_size: WindowSize) -> WindowSize {
		let axis_size = window_size.0.min(window_size.1);
		(axis_size, axis_size)
	}

	const fn get(&self) -> &SpinitronStateData {
		self.continually_updated.get_curr_data()
	}

	//////////

	pub const fn model_texture_was_updated(&self, model_name: SpinitronModelName) -> bool {
		self.just_got_new_continual_data && self.get().cached_model_data[model_name as usize].texture_creation_info_hash_changed
	}

	pub const fn model_text_was_updated(&self, model_name: SpinitronModelName) -> bool {
		self.just_got_new_continual_data && self.get().cached_model_data[model_name as usize].string_changed
	}

	pub fn get_cached_texture_creation_info(&self, model_name: SpinitronModelName) -> TextureCreationInfo {
		let bytes = &self.get().cached_model_data[model_name as usize].texture_bytes;
		TextureCreationInfo::RawBytes(Cow::Borrowed(bytes))
	}

	// Not returning a cached `TextureCreationInfo` for text, since that's created on-the-fly by the client of `SpinitronState`
	pub fn get_cached_model_text(&self, model_name: SpinitronModelName) -> &str {
		&self.get().cached_model_data[model_name as usize].string
	}

	//////////

	pub fn get_historic_spin_at_index(&mut self, spin_index: usize, spin_history_window_size: WindowSize) -> Option<TextureHandle> {
		self.spin_history_texture_size = Self::make_correct_texture_size_from_window_size(spin_history_window_size);
		self.history_list_texture_manager.get_texture_at_index(spin_index, &self.get().spin_history_list)
	}

	pub fn update(&mut self, spin_window_size: WindowSize, texture_pool: &mut TexturePool, error_state: &mut ErrorState) -> MaybeError {
		let spin_texture_size = Self::make_correct_texture_size_from_window_size(spin_window_size);

		//////////

		let continual_state = self.continually_updated.update(
			&(spin_texture_size, self.spin_history_texture_size), error_state
		);

		self.just_got_new_continual_data = continual_state == ContinuallyUpdatedState::GotNewData;

		if !self.just_got_new_continual_data {
			return Ok(());
		}

		//////////

		let Self {
			/* We have to do this ugly destructuring for the Rust compiler to accept
			the 2 struct fields being borrowed both mutably and immutably at the same time */
			history_list_texture_manager, ..
		} = self;

		history_list_texture_manager.update_from_history_list(
			// I can't use the `get` function here, and it's unclear why...
			&self.continually_updated.get_curr_data().spin_history_list,
			texture_pool
		)
	}
}
