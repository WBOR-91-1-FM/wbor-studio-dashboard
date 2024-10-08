use std::borrow::Cow;

use chrono::Timelike;
use isahc::AsyncReadResponseExt;

use crate::{
	request,
	texture::TextureCreationInfo,
	dashboard_defs::error::ErrorState,

	utility_types::{
		generic_result::*,
		continually_updated::{Updatable, ContinuallyUpdated}
	},

	spinitron::model::{
		NUM_SPINITRON_MODEL_TYPES,
		Spin, Playlist, Persona, Show,
		SpinitronModel, SpinitronModelName
	}
};

//////////

#[derive(Clone, PartialEq)]
pub enum ModelAgeState {
	BeforeIt,
	CurrentlyActive,
	AfterIt,
	AfterItFromCustomExpiryDuration
}

#[derive(Clone)]
struct ModelAgeData {
	custom_expiry_duration: chrono::Duration,
	curr_age_state: ModelAgeState,
	just_updated_state: bool
}

impl ModelAgeData {
	fn new(custom_expiry_duration: chrono::Duration, model: &dyn SpinitronModel) -> GenericResult<Self> {
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
			let curr_time = chrono::Utc::now();

			let time_after_start = curr_time.signed_duration_since(start_time);
			let time_after_end = curr_time.signed_duration_since(end_time);

			let zero = chrono::Duration::zero();

			// The custom end may be before or after the actual end
			let (is_after_start, is_after_end, is_after_custom_end) = (
				time_after_start > zero,
				time_after_end > zero,
				time_after_end > self.custom_expiry_duration
			);

			let new_age_state = if is_after_start {
				if is_after_custom_end {
					/* The first branch is for when the custom expiry is before the
					actual end time; so then, give priority to the actual end */
					if is_after_end && self.custom_expiry_duration < zero {ModelAgeState::AfterIt}
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

//////////

#[derive(Clone)]
struct SpinitronStateData {
	api_key: String,
	get_fallback_texture_creation_info: fn() -> TextureCreationInfo<'static>,

	spin: Spin,
	playlist: Playlist,
	persona: Persona,
	show: Show,

	age_data: [ModelAgeData; NUM_SPINITRON_MODEL_TYPES],
	precached_texture_bytes: [Vec<u8>; NUM_SPINITRON_MODEL_TYPES],

	/* The boolean at index `i` is true if the model at index `i` was recently
	updated. Model indices are (in order) spin, playlist, persona, and show. */
	update_statuses: [bool; NUM_SPINITRON_MODEL_TYPES]
}

type WindowSize = (u32, u32);

// The third param is the fallback texture creation info, and the fourth one is the spin window size
type SpinitronStateDataParams<'a> = (
	&'a str, // API key
	fn() -> TextureCreationInfo<'static>, // Fallback texture creation info getter
	[chrono::Duration; NUM_SPINITRON_MODEL_TYPES], // Custom model expiry durations
	WindowSize
);

//////////

impl SpinitronStateData {
	async fn new((api_key, get_fallback_texture_creation_info,
		custom_model_expiry_durations, spin_texture_size):
		SpinitronStateDataParams<'_>) -> GenericResult<Self> {

		////////// Getting the models

		let (spin, playlist, show) = futures::try_join!(
			Spin::get(api_key), Playlist::get(api_key), Show::get(api_key)
		)?;

		let persona = Persona::get(api_key, &playlist).await?;

		////////// Setting up their age data

		// TODO: once `zip` is implemented for arrays, rewrite this ugly bit
		let models_with_custom_expiry_durations: [(&dyn SpinitronModel, chrono::Duration); NUM_SPINITRON_MODEL_TYPES] = [
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

		const INITIAL_PRECACHED: Vec<u8> = Vec::new();

		let mut data = Self {
			api_key: api_key.to_owned(),
			get_fallback_texture_creation_info,

			spin, playlist, persona, show,

			age_data,
			precached_texture_bytes: [INITIAL_PRECACHED; NUM_SPINITRON_MODEL_TYPES],
			update_statuses: [false; NUM_SPINITRON_MODEL_TYPES]
		};

		////////// Getting the precached texture bytes

		let model_names = data.get_model_names();

		let model_texture_byte_futures = model_names.iter().map(
			|model_name| data.get_model_texture_bytes(*model_name, spin_texture_size)
		);

		let model_texture_bytes = futures::future::try_join_all(model_texture_byte_futures).await?;

		for (i, texture_bytes) in model_texture_bytes.iter().enumerate() {
			data.precached_texture_bytes[i] = texture_bytes.clone();
		}

		//////////

		Ok(data)
	}

	async fn get_model_texture_bytes(&self, model_name: SpinitronModelName, spin_texture_size: WindowSize) -> GenericResult<Vec<u8>> {
		async fn load_for_info(info: Cow<'_, TextureCreationInfo<'_>>) -> GenericResult<Vec<u8>> {
			/* I am doing this to speed up the loading of textures on the main
			thread, by doing the image URL requesting on this task/thread instead,
			and precaching anything from disk in byte form as well. */
			match info.as_ref() {
				TextureCreationInfo::Path(path) =>
					async_std::fs::read(path as &str).await.to_generic(),

				TextureCreationInfo::Url(url) => {
					let mut response = request::get(url).await?;
					Ok(response.bytes().await?)
				}

				TextureCreationInfo::RawBytes(_) =>
					panic!("Spinitron model textures should not be returning raw bytes!"),

				TextureCreationInfo::Text(_) =>
					panic!("Precaching the text texture creation info is not supported for plain Spinitron model textures!")
			}
		}

		let age_state = self.age_data[model_name as usize].curr_age_state.clone();
		let model = self.get_model_by_name(model_name);
		let get_fallback = || Cow::Owned((self.get_fallback_texture_creation_info)());

		let info = match model.get_texture_creation_info(age_state, spin_texture_size) {
			Some(texture_creation_info) => Cow::Owned(texture_creation_info),
			None => get_fallback()
		};

		match load_for_info(info).await {
			Ok(info) => Ok(info),

			Err(err) => {
				log::warn!("Reverting to fallback texture for Spinitron model. Error: '{err}'");
				load_for_info(get_fallback()).await
			}
		}
	}

	const fn get_models(&self) ->  [&dyn SpinitronModel; NUM_SPINITRON_MODEL_TYPES] {
		[&self.spin, &self.playlist, &self.persona, &self.show]
	}

	const fn get_model_names(&self) -> [SpinitronModelName; NUM_SPINITRON_MODEL_TYPES] {
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

		// Step 1: get the current spin.
		let maybe_new_spin = Spin::get(api_key).await?;

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

		let curr_minutes = chrono::Local::now().minute();

		// Shows can only be scheduled under 30-minute intervals (will not switch immediately if added sporadically)
		if curr_minutes == 0 || curr_minutes == 30 {
			/* Step 4: get the current show id (based on what's on the
			schedule, irrespective of what show was last on).
			This is not in the branch above, since the show should
			change directly on schedule, not when a new playlist is made. */
			self.show = Show::get(api_key).await?;
		}

		Ok(())
	}
}

impl Updatable for SpinitronStateData {
	type Param = WindowSize;

	async fn update(&mut self, spin_texture_size: &Self::Param) -> MaybeError {
		////////// Update the models

		let get_model_ids = |data: &Self|
			data.get_models().map(|model| model.get_id());

		let original_ids = get_model_ids(self);
		self.sync_models().await?;
		let new_ids = get_model_ids(self);

		////////// Update the model textures

		// TODO: how to do this without all the indexing?
		for model_name in self.get_model_names() {
			let i = model_name as usize;
			self.age_data[i] = self.age_data[i].clone().update(self.get_model_by_name(model_name))?;

			let updated = original_ids[i] != new_ids[i] || self.age_data[i].just_updated_state;

			if updated {
				self.precached_texture_bytes[i] = self.get_model_texture_bytes(
					model_name, *spin_texture_size
				).await?;
			}

			self.update_statuses[i] = updated;
		}

		Ok(())
	}
}

//////////

pub struct SpinitronState {
	continually_updated: ContinuallyUpdated<SpinitronStateData>
}

impl SpinitronState {
	pub async fn new(params: SpinitronStateDataParams<'_>) -> GenericResult<Self> {
		let data = SpinitronStateData::new(params).await?;

		let initial_spin_texture_size_guess = params.3;

		Ok(Self {
			continually_updated: ContinuallyUpdated::new(&data, &initial_spin_texture_size_guess, "Spinitron")
		})
	}

	const fn get(&self) -> &SpinitronStateData {
		self.continually_updated.get_data()
	}

	pub fn get_model_age_info(&self, model_name: SpinitronModelName) -> (bool, ModelAgeState) {
		let age_data = &self.get().age_data[model_name as usize];
		(age_data.just_updated_state, age_data.curr_age_state.clone())
	}

	pub const fn model_was_updated(&self, model_name: SpinitronModelName) -> bool {
		self.get().update_statuses[model_name as usize]
	}

	pub fn model_to_string(&self, model_name: SpinitronModelName) -> Cow<str> {
		let age_state = self.get_model_age_info(model_name).1;
		self.get().get_model_by_name(model_name).to_string(age_state)
	}

	// Note: this is not for text textures.
	pub fn get_cached_texture_creation_info(&self, model_name: SpinitronModelName) -> TextureCreationInfo {
		let bytes = &self.get().precached_texture_bytes[model_name as usize];
		TextureCreationInfo::RawBytes(Cow::Borrowed(bytes))
	}

	pub fn update(&mut self, spin_texture_size: WindowSize, error_state: &mut ErrorState) -> GenericResult<bool> {
		self.continually_updated.update(&spin_texture_size, error_state)
	}
}
