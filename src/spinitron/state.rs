use std::{
	sync::Arc,
	borrow::Cow
};

use crate::{
	window_tree::PixelAreaSDL,
	dashboard_defs::error::ErrorState,
	texture::pool::{TexturePool, TextureHandle, TextureCreationInfo},

	spinitron::{
		wrapper_types::SpinitronModelId,

		model::{
			SpinitronModel,
			SpinitronModelName,
			SpinitronModelWithProps,
			ModelTextureCreationInfo,
			NUM_SPINITRON_MODEL_TYPES,
			Spin, Playlist, Persona, Show
		}
	},

	utils::{
		ipc::*,
		time::*,
		request,
		file_utils,
		hash::hash_obj,
		generic_result::*,
		continually_updated::{ContinuallyUpdatable, ContinuallyUpdated, ContinuallyUpdatedState},
		api_history_list::{ApiHistoryList, ApiHistoryListImplementer, ApiHistoryListTextureManager}
	}
};

////////// Model age stuff:

#[derive(Copy, Clone, PartialEq)]
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
	fn new(custom_expiry_duration: Duration) -> Self {
		Self {
			custom_expiry_duration,
			curr_age_state: ModelAgeState::CurrentlyActive,
			just_updated_state: false
		}
	}

	fn update<Model: SpinitronModel + ?Sized>(&mut self, model: &Model) {
		if let (start_time, Some(end_time)) = model.get_time_range() {
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
		else {
			self.just_updated_state = false; // TODO: handle this more properly later on!
		}
	}
}

////////// The implementer for the spin history list:

#[derive(Clone)]
struct SpinHistoryListImplementer {}

struct SpinHistoryListImplementerParam {
	get_fallback_texture_path: fn() -> &'static str,
	item_texture_size: SpinHistoryItemTextureSize
}

impl ApiHistoryListImplementer for SpinHistoryListImplementer {
	type Key = SpinitronModelId;
	type NonNative = Spin;
	type Native = Arc<Spin>;

	type Param = SpinHistoryListImplementerParam;
	type ResolveTextureCreationInfoParam = ();
	type IntermediateTextureCreationInfo = Arc<[u8]>;

	fn may_need_to_sort_api_results() -> bool {false /* Spins come in order! */}
	fn compare(a: &Self::NonNative, b: &Self::NonNative) -> std::cmp::Ordering {a.get_start_time().cmp(&b.get_start_time())}

	fn get_key(offshore: &Self::NonNative) -> Self::Key {offshore.get_id()}
	fn is_expired(_: &Self::Param, _: &Self::NonNative) -> bool {false /* Spins don't expire in the history list */}

	fn create_new_local(_: &Self::Param, offshore: &Self::NonNative) -> Self::Native {Arc::new(offshore.clone())}

	fn update_local(param: &Self::Param, _: &mut Self::Native) -> bool {
		// Only updating local if the true texture size was just found
		param.item_texture_size.just_found_true_size
	}

	async fn get_intermediate_texture_creation_info(param: &Self::Param, local: &Self::Native) -> Self::IntermediateTextureCreationInfo {
		let maybe_info = local.get_texture_creation_info(ModelAgeState::CurrentlyActive, param.item_texture_size.size);
		get_model_texture_bytes(&maybe_info, param.get_fallback_texture_path).await
	}

	fn resolve_texture_creation_info<'a>(_: &(), _: &Self::Native,
		intermediate_texture_creation_info: &'a Self::IntermediateTextureCreationInfo) -> TextureCreationInfo<'a> {

		TextureCreationInfo::RawBytes(Cow::Borrowed(intermediate_texture_creation_info))
	}
}

//////////

#[derive(Copy, Clone)]
struct SpinHistoryItemTextureSize {
	size: PixelAreaSDL,
	just_found_true_size: bool
}

impl SpinHistoryItemTextureSize {
	fn new(size: PixelAreaSDL) -> Self {
		Self {
			size,
			just_found_true_size: false
		}
	}

	fn update(&mut self, new_size: PixelAreaSDL) {
		self.just_found_true_size = self.size != new_size;
		self.size = new_size;
	}
}

////////// Defining some types pertaining to `SpinitronStateData`

#[derive(Clone, Default)]
struct ModelDataCacheEntry {
	texture_bytes: Arc<[u8]>, // This is an `Arc` to avoid the cost of copying
	texture_creation_info_hash: u64,
	texture_creation_info_hash_changed: bool,

	string: Arc<Cow<'static, str>>,
	string_changed: bool
}

impl ModelDataCacheEntry {
	fn invalidate(&mut self) {
		self.texture_creation_info_hash_changed = false;
		self.string_changed = false;
	}

	async fn update<Model: SpinitronModelWithProps>(
		&mut self, age_state: ModelAgeState,
		model: &Model, spin_texture_size: PixelAreaSDL,
		get_fallback_texture_path: fn() -> &'static str) {

		let maybe_new_model_string = model.to_string(age_state);
		let texture_creation_info = model.get_texture_creation_info(age_state, spin_texture_size);

		////////// Finding an appropriate hash for the texture creation info

		let mut to_hash = Cow::Borrowed(&texture_creation_info);

		/* Sometimes, image URLs will have the format of `https://is*-ssl.mzstatic.com/image/thumb/...`, where `*` is a number.
		The issue is that the number may change, and the image will still be the same. The result of this situation is an image transitioning
		to itself, which looks wrong on-screen. We want to avoid this situation, so we cut off everything before the ".com" part as a part of a
		psuedo-URL to get a hash that doesn't indicate different images for a just slightly different URL. TODO: solve the first-on-screen case for this
		bug too, which happens when the spin age state changes to `ModelAgeState::AfterIt`, and a proper texture size for the first spin has been found
		(resulting in another false transition). */
		if let ModelTextureCreationInfo::Url(url) = &texture_creation_info {
			const CUTOFF: &str = ".com";
			let dot_com_point = url.find(CUTOFF);

			if let Some(dcp) = dot_com_point {
				let url_slice = &url[dcp + CUTOFF.len()..];
				to_hash = Cow::Owned(ModelTextureCreationInfo::Url(Cow::Borrowed(url_slice)));
			}
			else if Model::NAME == SpinitronModelName::Spin {
				panic!("The Spinitron image spin URL structure fundamentally changed! The URL in question is: '{url:?}'");
			}
		}

		let texture_creation_info_hash = hash_obj(&to_hash);

		////////// Updating the cache entry

		/* Comparing hashes means that we don't need to store the old `TextureCreationInfo`; but it does mean that we have to
		iterate over the new `TextureCreationInfo` with no early returns (which would've been possible if doing direct comparisons). */
		self.texture_creation_info_hash_changed = texture_creation_info_hash != self.texture_creation_info_hash;

		if self.texture_creation_info_hash_changed {
			self.texture_bytes = get_model_texture_bytes(&texture_creation_info, get_fallback_texture_path).await;
		}

		self.string_changed = maybe_new_model_string != *self.string;

		if self.string_changed {
			self.string = Arc::new(maybe_new_model_string);
		}
	}
}

//////////

#[derive(Clone)]
struct SpinitronModelEntry<Model: SpinitronModel> {
	model: Arc<Model>,
	age_data: ModelAgeData,
	cached_data: ModelDataCacheEntry
}

impl<Model: SpinitronModelWithProps> SpinitronModelEntry<Model> {
	async fn update_age_state_and_cache(&mut self, id_changed: bool,
		spin_texture_size: PixelAreaSDL, get_fallback_texture_path: &fn() -> &'static str) {

		let model = self.model.as_ref();

		self.age_data.update(model);

		if id_changed || self.age_data.just_updated_state {
			self.cached_data.update(
				self.age_data.curr_age_state,
				model,
				spin_texture_size,
				*get_fallback_texture_path
			).await;
		}
	}
}

//////////

#[derive(Clone)]
struct SpinitronStateData {
	api_key: Arc<String>,

	spin_entry: SpinitronModelEntry<Spin>,
	playlist_entry: SpinitronModelEntry<Playlist>,
	persona_entry: SpinitronModelEntry<Persona>,
	show_entry: SpinitronModelEntry<Show>,

	get_fallback_texture_path: fn() -> &'static str,
	spin_history_item_texture_size: SpinHistoryItemTextureSize,
	spin_history_list: ApiHistoryList<SpinHistoryListImplementer>
}

// The third param is the fallback texture creation info, and the fourth one is the spin window size
type SpinitronStateDataParams<'a> = (
	&'a str, // API key
	fn() -> &'static str, // Fallback texture path getter
	Duration, // The API update rate
	[Duration; NUM_SPINITRON_MODEL_TYPES], // Custom model expiry durations
	PixelAreaSDL, // The spin texture size (for the primary spin)
	PixelAreaSDL, // The spin history item texture size
	usize // The number of spins shown in the history
);

//////////

// This is expected to never fail (the fallback must succeed)
async fn get_model_texture_bytes(
	texture_creation_info: &ModelTextureCreationInfo<'_>,
	get_fallback_texture_path: fn() -> &'static str) -> Arc<[u8]> {

	let get_fallback = |maybe_err| async {
		if let Some(err) = maybe_err {
			log::warn!("Reverting to fallback texture for Spinitron model. Error: '{err}'");
		}

		let fallback_path = get_fallback_texture_path();
		let bytes = file_utils::read_file_contents(fallback_path).await.expect("Fallback texture path failed!");
		Arc::from(bytes)
	};

	match texture_creation_info {
		ModelTextureCreationInfo::Nothing => {
			get_fallback(None).await
		}

		ModelTextureCreationInfo::Path(path) => {
			match file_utils::read_file_contents(path).await {
				Ok(bytes) => Arc::from(bytes),
				Err(err) => get_fallback(Some(err)).await
			}
		}

		ModelTextureCreationInfo::Url(url) => {
			match request::get(url, None).await {
				Ok(response) => {
					match response.bytes().await {
						Ok(bytes) => Arc::from(bytes.to_vec()),
						Err(err) => get_fallback(Some(err.into())).await
					}
				}
				Err(err) => get_fallback(Some(err)).await
			}

		}
	}
}

//////////

impl SpinitronStateData {
	fn new((api_key, get_fallback_texture_path, _,
		custom_model_expiry_durations, _, spin_history_item_texture_size,
		num_spins_shown_in_history): SpinitronStateDataParams) -> Self {

		//////////

		fn entry<Model: SpinitronModelWithProps>(
			custom_model_expiry_durations: &[Duration; NUM_SPINITRON_MODEL_TYPES]
		) -> SpinitronModelEntry<Model> {

			SpinitronModelEntry {
				model: Arc::new(Model::default()),
				age_data: ModelAgeData::new(custom_model_expiry_durations[Model::NAME as usize]),
				cached_data: ModelDataCacheEntry::default()
			}
		}

		//////////

		Self {
			api_key: Arc::new(api_key.to_owned()),

			spin_entry: entry::<Spin>(&custom_model_expiry_durations),
			playlist_entry: entry::<Playlist>(&custom_model_expiry_durations),
			persona_entry: entry::<Persona>(&custom_model_expiry_durations),
			show_entry: entry::<Show>(&custom_model_expiry_durations),

			get_fallback_texture_path,
			spin_history_item_texture_size: SpinHistoryItemTextureSize::new(spin_history_item_texture_size),
			spin_history_list: ApiHistoryList::new(num_spins_shown_in_history)
		}
	}
}

impl ContinuallyUpdatable for SpinitronStateData {
	type Param = (PixelAreaSDL, PixelAreaSDL);

	async fn update(&mut self, (spin_texture_size, spin_history_item_texture_size): &Self::Param) -> MaybeError {
		let Self {
			api_key,

			spin_entry, playlist_entry,
			persona_entry, show_entry,

			get_fallback_texture_path,
			spin_history_list,
			..
		} = self;

		// Syncing the internal item texture size with the external one
		self.spin_history_item_texture_size.update(*spin_history_item_texture_size);

		spin_entry.cached_data.invalidate(); playlist_entry.cached_data.invalidate();
		persona_entry.cached_data.invalidate(); show_entry.cached_data.invalidate();

		////////// Defining the spin future

		let spin_future = async {
			let mut current_spin_and_history = Spin::get_current_and_history(
				api_key, spin_history_list.get_max_items()
			).await?;

			let maybe_new_spin = &current_spin_and_history[0];
			let spin_id_changed = maybe_new_spin.get_id() != spin_entry.model.get_id();

			if spin_id_changed {
				spin_entry.model = Arc::new(maybe_new_spin.clone());
			}

			tokio::join!(
				spin_entry.update_age_state_and_cache(spin_id_changed, *spin_texture_size, get_fallback_texture_path),

				spin_history_list.update(&mut current_spin_and_history[1..], SpinHistoryListImplementerParam {
					get_fallback_texture_path: *get_fallback_texture_path,
					item_texture_size: self.spin_history_item_texture_size
				})
			);

			let result: MaybeError = Ok(());
			result
		};

		////////// Defining the playlist/persona future

		let playlist_and_persona_future = async {
			let old_playlist_id = playlist_entry.model.get_id();
			let maybe_new_playlist = Playlist::get(api_key).await?;

			let playlist_id_changed = maybe_new_playlist.get_id() != old_playlist_id;

			if playlist_id_changed {
				playlist_entry.model = maybe_new_playlist.clone();
			}

			let playlist = playlist_entry.model.clone();

			tokio::try_join!(
				async {
					playlist_entry.update_age_state_and_cache(
						playlist_id_changed, *spin_texture_size, get_fallback_texture_path
					).await;

					Ok(())
				},

				async {
					let old_persona_id = persona_entry.model.get_id();

					if playlist_id_changed {
						persona_entry.model = Persona::get(api_key, &playlist).await?;
					}

					persona_entry.update_age_state_and_cache(
						old_persona_id != persona_entry.model.get_id(),
						*spin_texture_size, get_fallback_texture_path
					).await;

					Ok(())
				}
			)
		};

		////////// Making a show future

		let show_future = async {
			let old_show_id = show_entry.model.get_id();
			let curr_minutes = get_local_time().minute();
			let show_not_initialized_yet = old_show_id == 0;

			// Shows can only be scheduled under 30-minute intervals (will not switch immediately if added sporadically)
			if curr_minutes == 0 || curr_minutes == 30 || show_not_initialized_yet {
				show_entry.model = Show::get(api_key).await?;
			}

			show_entry.update_age_state_and_cache(
				old_show_id != show_entry.model.get_id(),
				*spin_texture_size, get_fallback_texture_path
			).await;

			Ok(())
		};

		////////// Joining on all of them

		tokio::try_join!(spin_future, playlist_and_persona_future, show_future)?;
		Ok(())
	}
}

//////////

pub struct SpinitronState {
	just_got_new_continual_data: bool,
	continually_updated: ContinuallyUpdated<SpinitronStateData>,
	instant_update_socket_listener: IpcSocketListener,

	spin_history_item_texture_size: PixelAreaSDL,
	history_list_texture_manager: ApiHistoryListTextureManager<SpinHistoryListImplementer>
}

impl SpinitronState {
	pub async fn new(params: SpinitronStateDataParams<'_>) -> GenericResult<Self> {
		let (.., api_update_rate, _, initial_spin_texture_size_guess,
			initial_spin_history_texture_size_guess, max_spin_history_items
		) = params;

		let data = SpinitronStateData::new(params);
		let texture_size_guesses = (initial_spin_texture_size_guess, initial_spin_history_texture_size_guess);

		let continually_updated = ContinuallyUpdated::new(
			data, texture_size_guesses, "Spinitron", api_update_rate
		);

		Ok(Self {
			just_got_new_continual_data: false,
			continually_updated,
			instant_update_socket_listener: make_ipc_socket_listener("spinitron_instant_update").await?,

			spin_history_item_texture_size: initial_spin_history_texture_size_guess,
			history_list_texture_manager: ApiHistoryListTextureManager::new(max_spin_history_items, None)
		})
	}

	fn make_correct_texture_size_from_window_size(window_size: PixelAreaSDL) -> PixelAreaSDL {
		let axis_size = window_size.0.min(window_size.1);
		(axis_size, axis_size)
	}

	const fn get(&self, model_name: SpinitronModelName) -> &ModelDataCacheEntry {
		let data = self.continually_updated.get_curr_data();

		match model_name {
			SpinitronModelName::Spin => &data.spin_entry.cached_data,
			SpinitronModelName::Playlist => &data.playlist_entry.cached_data,
			SpinitronModelName::Persona => &data.persona_entry.cached_data,
			SpinitronModelName::Show => &data.show_entry.cached_data
		}
	}

	//////////

	pub const fn model_texture_was_updated(&self, model_name: SpinitronModelName) -> bool {
		self.just_got_new_continual_data && self.get(model_name).texture_creation_info_hash_changed
	}

	pub const fn model_text_was_updated(&self, model_name: SpinitronModelName) -> bool {
		self.just_got_new_continual_data && self.get(model_name).string_changed
	}

	pub fn get_cached_texture_creation_info(&self, model_name: SpinitronModelName) -> TextureCreationInfo {
		let bytes = &self.get(model_name).texture_bytes;
		TextureCreationInfo::RawBytes(Cow::Borrowed(bytes))
	}

	// Not returning a cached `TextureCreationInfo` for text, since that's created on-the-fly by the client of `SpinitronState`
	pub fn get_cached_model_text(&self, model_name: SpinitronModelName) -> &str {
		&self.get(model_name).string
	}

	//////////

	pub fn get_historic_spin_at_index(&mut self, spin_index: usize, spin_history_window_size: PixelAreaSDL) -> Option<TextureHandle> {
		self.spin_history_item_texture_size = Self::make_correct_texture_size_from_window_size(spin_history_window_size);
		self.history_list_texture_manager.get_texture_at_index(spin_index, &self.continually_updated.get_curr_data().spin_history_list)
	}

	pub fn update(&mut self, spin_window_size: PixelAreaSDL, texture_pool: &mut TexturePool, error_state: &mut ErrorState) {
		let spin_texture_size = Self::make_correct_texture_size_from_window_size(spin_window_size);
		let continual_param = (spin_texture_size, self.spin_history_item_texture_size);

		////////// Check for an instant wakeup, and check if we got new continual data or not

		if try_listening_to_ipc_socket(&mut self.instant_update_socket_listener).is_some() {
			// The result of this wakeup may take until the next update iteration to be processed
			self.continually_updated.wake_up_if_sleeping();
		}

		let continual_state = self.continually_updated.update(continual_param, error_state);
		self.just_got_new_continual_data = continual_state == ContinuallyUpdatedState::GotNewData;
		if !self.just_got_new_continual_data {return;}

		//////////

		let Self {
			/* We have to do this ugly destructuring for the Rust compiler to accept
			the 2 struct fields being borrowed both mutably and immutably at the same time
			(note: the immutably borrowed field is the `self.continually_updated` below) */
			history_list_texture_manager, ..
		} = self;

		history_list_texture_manager.update_from_history_list(
			&self.continually_updated.get_curr_data().spin_history_list,
			texture_pool,
			&()
		);
	}
}
