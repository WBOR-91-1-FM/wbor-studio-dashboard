use crate::{
	utility_types::{
		generic_result::{GenericResult, MaybeError},
		thread_task::{Updatable, ContinuallyUpdated}
	},

	spinitron::model::{
		NUM_SPINITRON_MODEL_TYPES,
		Spin, Playlist, Persona, Show,
		SpinitronModel, SpinitronModelName
	}
};

#[derive(Clone)]
struct SpinExpiryData {
	expiry_duration: chrono::Duration,
	end_time: chrono::DateTime<chrono::Utc>,
	marked_as_expired: bool,
	just_expired: bool
}

impl SpinExpiryData {
	fn new(expiry_duration: chrono::Duration, spin: &Spin) -> GenericResult<Self> {
		let mut data = Self {
			expiry_duration,
			end_time: chrono::DateTime::<chrono::Utc>::MIN_UTC,
			marked_as_expired: false,
			just_expired: false
		};

		data.mark_expiration(spin)?;
		Ok(data)
	}

	fn mark_expiration(&mut self, spin: &Spin) -> MaybeError {
		self.end_time = spin.get_end_time()?;

		let curr_time = chrono::Utc::now();
		let time_after_end = curr_time.signed_duration_since(self.end_time);

		/*
		if time_after_end.num_microseconds() < Some(0) {
			println!("This spin is currently ongoing/in-progress!");
		}
		*/

		let marked_before = self.marked_as_expired;
		self.marked_as_expired = time_after_end > self.expiry_duration;
		self.just_expired = !marked_before && self.marked_as_expired;

		Ok(())
	}
}

#[derive(Clone)]
struct SpinitronStateData {
	spin: Spin,
	playlist: Playlist,
	persona: Persona,
	show: Show,

	spin_expiry_data: SpinExpiryData,

	api_key: String,

	/* The boolean at index `i` is true if the model at index `i` was recently
	updated. Model indices are (in order) spin, playlist, persona, and show. */
	update_status: [bool; NUM_SPINITRON_MODEL_TYPES]
}

type SpinitronStateDataParams<'a> = (&'a str, chrono::Duration);

impl SpinitronStateData {
	fn new((api_key, spin_expiry_duration): SpinitronStateDataParams) -> GenericResult<Self> {
		let spin = Spin::get(api_key)?;
		let playlist = Playlist::get(api_key)?;
		let persona =  Persona::get(api_key, &playlist)?;
		let show = Show::get(api_key)?;

		let spin_expiry_data = SpinExpiryData::new(spin_expiry_duration, &spin)?;

		Ok(Self {
			spin, playlist, persona, show,
			spin_expiry_data,
			api_key: api_key.to_string(),
			update_status: [false, false, false, false]
		})
	}

	fn sync_models(&mut self) -> MaybeError {
		let api_key = &self.api_key;

		// Step 1: get the current spin.
		let maybe_new_spin = Spin::get(api_key)?;

		if maybe_new_spin.get_id() != self.spin.get_id() {
			self.spin = maybe_new_spin;
		}

		// Step 2: mark the expiration of the current spin.
		self.spin_expiry_data.mark_expiration(&self.spin)?;

		/* Step 3: get a maybe new playlist (don't base it on a spin ID,
		since the spin may not belong to a playlist under automation). */
		let maybe_new_playlist = Playlist::get(api_key)?;

		if maybe_new_playlist.get_id() != self.playlist.get_id() {
			/* Step 4: get the persona id based on the playlist id (since otherwise, you'll
			just get some persona that's first in Spinitron's internal list of personas. */
			self.persona = Persona::get(api_key, &maybe_new_playlist)?;
			self.playlist = maybe_new_playlist;
		}

		/* Step 5: get the current show id (based on what's on the
		schedule, irrespective of what show was last on).
		TODO: should I only do this in the branch above? */
		self.show = Show::get(api_key)?;

		Ok(())
	}
}

impl Updatable for SpinitronStateData {
	fn update(&mut self) -> MaybeError {
		// TODO: do a mapping here instead
		let get_model_ids = |data: &SpinitronStateData| [
			data.spin.get_id(), data.playlist.get_id(),
			data.persona.get_id(), data.show.get_id()
		];

		let original_ids = get_model_ids(self);
		self.sync_models()?;
		let new_ids = get_model_ids(self);

		for ((status, original_id), new_id) in self.update_status
			.iter_mut().zip(original_ids).zip(new_ids) {

			*status = original_id != new_id;
		}

		Ok(())
	}
}

//////////

pub struct SpinitronState {
	continually_updated: ContinuallyUpdated<SpinitronStateData>
}

impl SpinitronState {
	pub fn new(params: SpinitronStateDataParams) -> GenericResult<Self> {
		let data = SpinitronStateData::new(params)?;
		Ok(Self {continually_updated: ContinuallyUpdated::new(&data, "Spinitron")})
	}

	pub fn get_model_by_name(&self, name: SpinitronModelName) -> &dyn SpinitronModel {
		let data = self.continually_updated.get_data();

		match name {
			SpinitronModelName::Spin => &data.spin,
			SpinitronModelName::Playlist => &data.playlist,
			SpinitronModelName::Persona => &data.persona,
			SpinitronModelName::Show => &data.show
		}
	}

	pub fn spin_just_expired(&self) -> bool {
		self.continually_updated.get_data().spin_expiry_data.just_expired
	}

	pub fn model_was_updated(&self, model_name: SpinitronModelName) -> bool {
		self.continually_updated.get_data().update_status[model_name as usize]
	}

	pub fn update(&mut self) -> GenericResult<bool> {
		self.continually_updated.update()
	}
}
