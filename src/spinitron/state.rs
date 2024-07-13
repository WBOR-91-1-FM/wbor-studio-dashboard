use std::borrow::Cow;

use chrono::Timelike;

use crate::{
	request,
	texture::TextureCreationInfo,

	utility_types::{
		generic_result::*,
		thread_task::{Updatable, ContinuallyUpdated}
	},

	spinitron::model::{
		NUM_SPINITRON_MODEL_TYPES,
		Spin, Playlist, Persona, Show,
		SpinitronModel, SpinitronModelName
	}
};

//////////

#[derive(Clone)]
struct ExpiryData {
	expiry_duration: chrono::Duration,
	end_time: chrono::DateTime<chrono::Utc>,
	marked_as_expired: bool,
	just_expired: bool
}

impl ExpiryData {
	fn new(expiry_duration: chrono::Duration, model: &dyn SpinitronModel) -> GenericResult<Self> {
		let data = Self {
			expiry_duration,
			end_time: chrono::DateTime::<chrono::Utc>::MIN_UTC,
			marked_as_expired: false,
			just_expired: false
		};

		data.mark_expiration(model)
	}

	// This returns the new expiry state
	fn mark_expiration(mut self, model: &dyn SpinitronModel) -> GenericResult<ExpiryData> {
		self.end_time = model.get_end_time()?;

		let curr_time = chrono::Utc::now();
		let time_after_end = curr_time.signed_duration_since(self.end_time);

		/*
		if time_after_end.num_microseconds() < Some(0) {
			println!("This model is currently ongoing/in-progress!");
		}
		*/

		let marked_before = self.marked_as_expired;
		self.marked_as_expired = time_after_end > self.expiry_duration;
		self.just_expired = !marked_before && self.marked_as_expired;

		Ok(self)
	}
}

//////////

#[derive(Clone)]
struct SpinitronStateData {
	api_key: String,
	fallback_texture_creation_info: &'static TextureCreationInfo<'static>,

	spin: Spin,
	playlist: Playlist,
	persona: Persona,
	show: Show,

	expiry_data: [ExpiryData; NUM_SPINITRON_MODEL_TYPES],
	precached_texture_bytes: [Vec<u8>; NUM_SPINITRON_MODEL_TYPES],

	/* The boolean at index `i` is true if the model at index `i` was recently
	updated. Model indices are (in order) spin, playlist, persona, and show. */
	update_statuses: [bool; NUM_SPINITRON_MODEL_TYPES]
}

type WindowSize = (u32, u32);
type SpinitronModels<'a> = [&'a dyn SpinitronModel; NUM_SPINITRON_MODEL_TYPES];

// The third param is the fallback texture creation info, and the fourth one is the spin window size
type SpinitronStateDataParams<'a> = (
	&'a str, // API key
	&'static TextureCreationInfo<'static>, // Fallback texture creation info
	[chrono::Duration; NUM_SPINITRON_MODEL_TYPES], // Model expiry durations
	WindowSize
);

//////////

impl SpinitronStateData {
	fn new((api_key, fallback_texture_creation_info,
		model_expiry_durations, spin_window_size):
		SpinitronStateDataParams) -> GenericResult<Self> {

		let spin = Spin::get(api_key)?;
		let playlist = Playlist::get(api_key)?;
		let persona = Persona::get(api_key, &playlist)?;

		/* Note: if no show is scheduled during the current time,
		this just gives you the next scheduled show. */
		let show = Show::get(api_key)?;

		// TODO: once `zip` is implemented for arrays, rewrite this ugly bit
		let models_with_expiry_durations: [(&dyn SpinitronModel, chrono::Duration); NUM_SPINITRON_MODEL_TYPES] = [
			(&spin, model_expiry_durations[0]),
			(&playlist, model_expiry_durations[1]),
			(&persona, model_expiry_durations[2]),
			(&show, model_expiry_durations[3])
		];

		// TODO: don't unwrap once `try_map` becomes stable
		let expiry_data = models_with_expiry_durations
			.map(|(model, expiry_duration)|
				ExpiryData::new(expiry_duration, model).unwrap()
			);

		const INITIAL_PRECACHED: Vec<u8> = Vec::new();

		let mut data = Self {
			api_key: api_key.to_owned(),
			fallback_texture_creation_info,

			spin, playlist, persona, show,

			expiry_data,
			precached_texture_bytes: [INITIAL_PRECACHED; NUM_SPINITRON_MODEL_TYPES],
			update_statuses: [false; NUM_SPINITRON_MODEL_TYPES]
		};

		data.precached_texture_bytes = data.get_models().map( // TODO: don't unwrap once `try_map` becomes stable
			|model| data.get_model_texture_bytes(model, spin_window_size).unwrap()
		);

		Ok(data)
	}

	fn get_model_texture_bytes(&self, model: &dyn SpinitronModel, size_pixels: WindowSize) -> GenericResult<Vec<u8>> {
		fn load_for_info(info: Cow<TextureCreationInfo>) -> GenericResult<Vec<u8>> {
			/* I am doing this to speed up the loading of textures on the main
			thread, by doing the image URL requesting on this thread instead,
			and precaching anything from disk in byte form as well. */
			match info.as_ref() {
				TextureCreationInfo::Path(path) =>
					std::fs::read(path as &str).to_generic(),

				TextureCreationInfo::Url(url) =>
					Ok(request::get(url)?.as_bytes().to_vec()),

				TextureCreationInfo::RawBytes(_) =>
					panic!("Spinitron model textures should not be returning raw bytes!"),

				TextureCreationInfo::Text(_) =>
					panic!("Precaching the text texture creation info is not supported for plain Spinitron model textures!")
			}
		}

		let info = match model.get_texture_creation_info(size_pixels) {
			Some(texture_creation_info) => Cow::Owned(texture_creation_info),
			None => Cow::Borrowed(self.fallback_texture_creation_info)
		};

		load_for_info(info).or_else(|error| {
			log::warn!("Reverting to fallback texture for Spinitron model. Error: '{error}'");
			load_for_info(Cow::Borrowed(self.fallback_texture_creation_info))
		})
	}

	const fn get_models(&self) -> SpinitronModels {
		[&self.spin, &self.playlist, &self.persona, &self.show]
	}

	fn sync_models(&mut self) -> MaybeError {
		let api_key = &self.api_key;

		// Step 1: get the current spin.
		let maybe_new_spin = Spin::get(api_key)?;

		if maybe_new_spin.get_id() != self.spin.get_id() {
			self.spin = maybe_new_spin;
		}

		//////////

		/* Step 2: get a maybe new playlist (don't base it on a spin ID,
		since the spin may not belong to a playlist under automation). */
		let maybe_new_playlist = Playlist::get(api_key)?;

		if maybe_new_playlist.get_id() != self.playlist.get_id() {
			/* Step 3: get the persona id based on the playlist id (since otherwise, you'll
			just get some persona that's first in Spinitron's internal list of personas. */
			self.persona = Persona::get(api_key, &maybe_new_playlist)?;
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
			self.show = Show::get(api_key)?;
		}

		Ok(())
	}
}

impl Updatable for SpinitronStateData {
	type Param = WindowSize;

	fn update(&mut self, param: &Self::Param) -> MaybeError {
		////////// Update the models

		let get_model_ids = |data: &Self|
			data.get_models().map(|model| model.get_id());

		let original_ids = get_model_ids(self);
		self.sync_models()?;
		let new_ids = get_model_ids(self);

		////////// Update the model textures

		// TODO: how to do this without all the indexing?
		for i in 0..NUM_SPINITRON_MODEL_TYPES {
			self.expiry_data[i] = self.expiry_data[i].clone().mark_expiration(self.get_models()[i])?;

			let updated = original_ids[i] != new_ids[i];

			if updated {
				let model = self.get_models()[i];
				self.precached_texture_bytes[i] = self.get_model_texture_bytes(model, *param)?;
			}

			self.update_statuses[i] = updated;
		}

		Ok(())
	}
}

//////////

pub struct SpinitronState {
	continually_updated: ContinuallyUpdated<SpinitronStateData>,
	saved_continually_updated_param: <SpinitronStateData as Updatable>::Param
}

impl SpinitronState {
	pub fn new(params: SpinitronStateDataParams) -> GenericResult<Self> {
		let data = SpinitronStateData::new(params)?;

		let initial_spin_window_size_guess = params.3;

		Ok(Self {
			continually_updated: ContinuallyUpdated::new(&data, &initial_spin_window_size_guess, "Spinitron"),
			saved_continually_updated_param: initial_spin_window_size_guess
		})
	}

	// TODO: should I use the `get_models` function here, perhaps?
	pub const fn get_model_by_name(&self, name: SpinitronModelName) -> &dyn SpinitronModel {
		let data = self.continually_updated.get_data();

		match name {
			SpinitronModelName::Spin => &data.spin,
			SpinitronModelName::Playlist => &data.playlist,
			SpinitronModelName::Persona => &data.persona,
			SpinitronModelName::Show => &data.show
		}
	}

	pub const fn model_just_expired(&self, model_name: SpinitronModelName) -> bool {
		self.continually_updated.get_data().expiry_data[model_name as usize].just_expired
	}

	pub const fn model_was_updated(&self, model_name: SpinitronModelName) -> bool {
		self.continually_updated.get_data().update_statuses[model_name as usize]
	}

	/* This is meant to be called by a spin texture window, so that the
	spin window size can be given to the continual updater (which preloads
	the spin texture's data on its line of execution, for less load times). */
	pub fn register_spin_window_size(&mut self, size: WindowSize) {
		self.saved_continually_updated_param = size;
	}

	// Note: this is not for text textures.
	pub fn get_cached_texture_creation_info(&self, model_name: SpinitronModelName) -> TextureCreationInfo {
		if self.model_just_expired(model_name) { // TODO: cache this info too
			self.get_model_by_name(model_name).get_texture_creation_info_when_expired()
		}
		else {
			let bytes = &self.continually_updated.get_data().precached_texture_bytes[model_name as usize];
			TextureCreationInfo::RawBytes(bytes)
		}
	}

	pub fn update(&mut self) -> GenericResult<bool> {
		self.continually_updated.update(&self.saved_continually_updated_param)
	}
}
