use std::{
	sync::Arc,
	borrow::Cow
};

use crate::{
	request,
	dashboard_defs::error::ErrorState,
	texture::pool::{TexturePool, TextureHandle, TextureCreationInfo, RemakeTransitionInfo},

	utility_types::{
		time::*,
		file_utils,
		hash::hash_obj,
		generic_result::*,
		continually_updated::{Updatable, ContinuallyUpdated},
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
	BeforeIt,
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

#[derive(Clone)]
struct ModelDataCacheEntry {
	texture_bytes: Arc<Vec<u8>>,
	texture_creation_info_hash: u64,
	texture_creation_info_hash_changed: bool,

	string: Arc<String>,
	string_changed: bool
}

impl ModelDataCacheEntry {
	fn empty() -> Self {
		Self {
			texture_bytes: Arc::new(Vec::new()),
			texture_creation_info_hash: 0,
			texture_creation_info_hash_changed: false,

			string: Arc::new(String::new()),
			string_changed: false
		}
	}
}

#[derive(Clone)]
struct SpinitronStateData {
	api_key: String,

	spin: Spin,
	playlist: Playlist,
	persona: Persona,
	show: Show,

	// TODO: perhaps merge these two
	age_data: [ModelAgeData; NUM_SPINITRON_MODEL_TYPES],
	cached_model_data: [ModelDataCacheEntry; NUM_SPINITRON_MODEL_TYPES],

	spin_history_list: APIHistoryList<SpinitronModelId, Spin, Spin, SpinHistoryListTraitImplementer>
}

// The third param is the fallback texture creation info, and the fourth one is the spin window size
type SpinitronStateDataParams<'a> = (
	&'a str, // API key
	fn() -> TextureCreationInfo<'static>, // Fallback texture creation info getter
	[Duration; NUM_SPINITRON_MODEL_TYPES], // Custom model expiry durations
	WindowSize, // The spin texture size (for the primary spin)
	WindowSize, // The spin history item texture size
	usize, // The number of spins shown in the history
	Option<RemakeTransitionInfo> // The optional remake transition info for spin history
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
	async fn new((api_key, get_fallback_texture_creation_info,
		custom_model_expiry_durations, spin_texture_size,
		spin_history_item_texture_size, num_spins_shown_in_history, _):
		SpinitronStateDataParams<'_>) -> GenericResult<Self> {

		////////// Getting the models, and updating the spin history list as well

		let ((spin, mut spin_history), playlist, show) = tokio::try_join!(
			Spin::get_current_and_history(api_key, num_spins_shown_in_history),
			Playlist::get(api_key), Show::get(api_key)
		)?;

		let mut spin_history_list = APIHistoryList::new(
			num_spins_shown_in_history,

			SpinHistoryListTraitImplementer {
				get_fallback_texture_creation_info,
				item_texture_size: spin_history_item_texture_size,
				just_found_true_texture_size: false
			}
		);

		let (persona, _) = tokio::try_join!(
			Persona::get(api_key, &playlist),

			// This can be skipped if you want faster loading
			spin_history_list.update(&mut spin_history)
		)?;

		////////// Setting up their age data

		// TODO: once `zip` is implemented for arrays, rewrite this ugly bit
		let models_with_custom_expiry_durations: [(&dyn SpinitronModel, Duration); NUM_SPINITRON_MODEL_TYPES] = [
			(&spin, custom_model_expiry_durations[0]),
			(&playlist, custom_model_expiry_durations[1]),
			(&persona, custom_model_expiry_durations[2]),
			(&show, custom_model_expiry_durations[3])
		];

		// TODO: don't unwrap once `try_map` becomes stable
		let age_data = models_with_custom_expiry_durations
			.map(|(model, custom_expiry_duration)|
				ModelAgeData::new(custom_expiry_duration, model).unwrap()
			);

		let mut data = Self {
			api_key: api_key.to_owned(),

			spin, playlist, persona, show,

			age_data,
			cached_model_data: std::array::from_fn(|_| ModelDataCacheEntry::empty()),

			spin_history_list
		};

		////////// Getting the precached texture bytes

		let model_names = Self::get_model_names();

		let futures = model_names.iter().map(
			|model_name| data.compute_cacheable_model_data(*model_name, spin_texture_size)
		);

		let all_cached = futures::future::join_all(futures).await;

		for (i, cached) in all_cached.iter().enumerate() {
			data.cached_model_data[i] = cached.clone();
		}

		//////////

		Ok(data)
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

	const fn get_models(&self) ->  [&dyn SpinitronModel; NUM_SPINITRON_MODEL_TYPES] {
		[&self.spin, &self.playlist, &self.persona, &self.show]
	}

	const fn get_model_names() -> [SpinitronModelName; NUM_SPINITRON_MODEL_TYPES] {
		[SpinitronModelName::Spin, SpinitronModelName::Playlist, SpinitronModelName::Persona, SpinitronModelName::Show]
	}

	pub const fn get_model_by_name(&self, model_name: SpinitronModelName) -> &dyn SpinitronModel {
		match model_name {
			SpinitronModelName::Spin => &self.spin,
			SpinitronModelName::Playlist => &self.playlist,
			SpinitronModelName::Persona => &self.persona,
			SpinitronModelName::Show => &self.show
		}
	}

	async fn sync_models(&mut self) -> MaybeError {
		let api_key = &self.api_key;

		// Step 1: get the current spin, and the spin history.
		let (maybe_new_spin, mut spin_history) = Spin::get_current_and_history(
			api_key, self.spin_history_list.get_max_items()
		).await?;

		if maybe_new_spin.get_id() != self.spin.get_id() {
			self.spin = maybe_new_spin;
		}

		//////////

		/* Step 2: get a maybe new playlist (don't base it on a spin ID,
		since the spin may not belong to a playlist under automation). */
		let maybe_new_playlist = Playlist::get(api_key).await?;

		if maybe_new_playlist.get_id() != self.playlist.get_id() {
			/* Step 3: get the persona id based on the playlist id (since otherwise, you'll
			just get some persona that's first in Spinitron's internal list of personas. */
			self.persona = Persona::get(api_key, &maybe_new_playlist).await?;
			self.playlist = maybe_new_playlist;
		}

		//////////

		let curr_minutes = get_local_time().minute();

		// Shows can only be scheduled under 30-minute intervals (will not switch immediately if added sporadically)
		if curr_minutes == 0 || curr_minutes == 30 {
			/* Step 4: get the current show id (based on what's on the
			schedule, irrespective of what show was last on).
			This is not in the branch above, since the show should
			change directly on schedule, not when a new playlist is made. */
			self.show = Show::get(api_key).await?;
		}

		// Step 5: update the spin history list.
		self.spin_history_list.update(&mut spin_history).await?;

		Ok(())
	}
}

impl Updatable for SpinitronStateData {
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

		////////// Update the model textures

		// TODO: how to do this without all the indexing?
		for model_name in Self::get_model_names() {
			let i = model_name as usize;
			self.age_data[i] = self.age_data[i].clone().update(self.get_model_by_name(model_name))?;

			// Under these conditions, the texture may have updated (sometimes, models will have the same texture across different IDs though)
			let maybe_updated = original_ids[i] != new_ids[i] || self.age_data[i].just_updated_state;

			if maybe_updated {
				self.cached_model_data[i] = self.compute_cacheable_model_data(model_name, *spin_texture_size).await;
			}
			else {
				let cache = &mut self.cached_model_data[i];
				cache.texture_creation_info_hash_changed = false; // Marking the texture as not updated
				cache.string_changed = false; // Marking the text as not updated
			}
		}

		Ok(())
	}
}

//////////

pub struct SpinitronState {
	continually_updated: ContinuallyUpdated<SpinitronStateData>,
	history_list_texture_manager: ApiHistoryListTextureManager<SpinitronModelId, Spin, Spin, SpinHistoryListTraitImplementer>,
	spin_history_texture_size: WindowSize
}

impl SpinitronState {
	pub async fn new(params: SpinitronStateDataParams<'_>) -> GenericResult<Self> {
		let (.., initial_spin_texture_size_guess, initial_spin_history_texture_size_guess,
			max_spin_history_items, maybe_remake_transition_info_for_spin_history
		) = params.clone();

		let data = SpinitronStateData::new(params).await?;
		let texture_size_guesses = (initial_spin_texture_size_guess, initial_spin_history_texture_size_guess);

		Ok(Self {
			continually_updated: ContinuallyUpdated::new(data, texture_size_guesses, "Spinitron").await,
			history_list_texture_manager: ApiHistoryListTextureManager::new(max_spin_history_items, maybe_remake_transition_info_for_spin_history),
			spin_history_texture_size: initial_spin_history_texture_size_guess
		})
	}

	fn make_correct_texture_size_from_window_size(window_size: WindowSize) -> WindowSize {
		let axis_size = window_size.0.min(window_size.1);
		(axis_size, axis_size)
	}

	const fn get(&self) -> &SpinitronStateData {
		self.continually_updated.get_data()
	}

	//////////

	pub const fn model_texture_was_updated(&self, model_name: SpinitronModelName) -> bool {
		self.get().cached_model_data[model_name as usize].texture_creation_info_hash_changed
	}

	pub fn get_cached_texture_creation_info(&self, model_name: SpinitronModelName) -> TextureCreationInfo {
		let bytes = &self.get().cached_model_data[model_name as usize].texture_bytes;
		TextureCreationInfo::RawBytes(Cow::Borrowed(bytes))
	}

	//////////

	pub const fn model_text_was_updated(&self, model_name: SpinitronModelName) -> bool {
		self.get().cached_model_data[model_name as usize].string_changed
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

	pub fn update(&mut self, spin_window_size: WindowSize, texture_pool: &mut TexturePool, error_state: &mut ErrorState) -> GenericResult<bool> {
		let spin_texture_size = Self::make_correct_texture_size_from_window_size(spin_window_size);

		let texture_sizes = (spin_texture_size, self.spin_history_texture_size);
		let continually_updated_result = self.continually_updated.update(&texture_sizes, error_state);

		let Self {
			/* We have to do this ugly destructuring for the Rust compiler to accept
			the 2 struct fields being borrowed both mutably and immutably at the same time */
			history_list_texture_manager, ..
		} = self;

		history_list_texture_manager.update_from_history_list(
			// I can't use the `get` function here, and it's unclear why...
			&self.continually_updated.get_data().spin_history_list,
			texture_pool
		)?;

		Ok(continually_updated_result)
	}
}
