use crate::{
	utility_types::{
		generic_result::{GenericResult, MaybeError},
		thread_task::{Updatable, ContinuallyUpdated}
	},

	spinitron::{
		api::{get_curr_spin, get_from_id},

		model::{
			Spin, Playlist, Persona, Show,
			SpinitronModel, SpinitronModelName,
			NUM_SPINITRON_MODEL_TYPES
		}
	}
};

#[derive(Clone)]
struct SpinitronStateData {
	spin: Spin,
	playlist: Playlist,
	persona: Persona,
	show: Show, // TODO: will there ever not be a show?

	api_key: String,

	/* The boolean at index `i` is true if the model at index `i` was recently
	updated. Model indices are (in order) spin, playlist, persona, and show. */
	update_status: [bool; NUM_SPINITRON_MODEL_TYPES]
}

impl SpinitronStateData {
	fn new(api_key: &str) -> GenericResult<Self> {
		// TODO: if there is no current spin, will this only return the last one?
		let spin = get_curr_spin(api_key)?;
		let mut playlist: Playlist = get_from_id(api_key, Some(spin.get_playlist_id()))?;
		let persona = get_from_id(api_key, Some(playlist.get_persona_id()))?;
		let show = Self::get_show_while_syncing_show_id(api_key, &mut playlist)?;

		/*
		let spin = Spin::default();
		let playlist = Playlist::default();
		let persona = Persona::default();
		let show = Show::default();
		*/

		Ok(Self {
			spin, playlist, persona, show,
			api_key: api_key.to_string(),
			update_status: [false, false, false, false]
		})
	}

	fn get_show_while_syncing_show_id(api_key: &str, playlist: &mut Playlist) -> GenericResult<Show> {
		let show: Show = get_from_id(api_key, playlist.get_show_id())?;

		/* It's possible that the playlist will not have a show id
		(e.g. if someone plays songs, without making a playlist to log them).
		In that case, this gets the current show according to the schedule,
		and the playlist's show ID is set after that. */

		if playlist.get_show_id().is_none() {
			log::info!("The playlist's show id was None, so setting it manually to the show id");
			playlist.set_show_id(show.get_id());
		}

		Ok(show)
	}
}

impl Updatable for SpinitronStateData {
	fn update(&mut self) -> MaybeError {
		let api_key = &self.api_key;
		let new_spin = get_curr_spin(api_key)?;

		let get_model_ids = |data: &SpinitronStateData| [
			data.spin.get_id(), data.playlist.get_id(),
			data.persona.get_id(), data.show.get_id()
		];

		let original_ids = get_model_ids(self);

		////////// TODO: make this less repetitive

		/* TODO:
		- Make a `sync` function for each of these instead?
		- Can a persona change without the spin changing? Maybe ignore that
		- Can I un-nest these series of `ifs`, into a series of `return`s?
		*/

		// Syncing the spin
		if self.spin.get_id() != new_spin.get_id() {

			let new_spin_playlist_id = new_spin.get_playlist_id();

			// Syncing the playlist
			if self.playlist.get_id() != new_spin_playlist_id {
				let mut new_playlist: Playlist = get_from_id(api_key, Some(new_spin_playlist_id))?;
				let new_playlist_persona_id = new_playlist.get_persona_id();

				// Syncing the persona
				if self.persona.get_id() != new_playlist_persona_id {
					self.persona = get_from_id(api_key, Some(new_playlist_persona_id))?;
				}

				////////// Syncing the show

				// If the playlist has a valid show id
				if let Some(new_playlist_show_id) = new_playlist.get_show_id() {
					// If the show id didn't match up, then refresh it
					if self.show.get_id() != new_playlist_show_id {
						log::info!("Do conventional refresh for show id");
						self.show = get_from_id(api_key, Some(new_playlist_show_id))?;
					}
				}
				else {
					/* In this case, the playlist didn't have a show id. This means that
					someone is playing music without logging it to a show's playlist.
					From this, the current show will be inferred based on the schedule,
					and the current playlist's show will be synced with the one from that show. */
					log::info!("Do unconventional refresh for show id");
					self.show = Self::get_show_while_syncing_show_id(api_key, &mut new_playlist)?;
				}

				self.playlist = new_playlist;
			}

			self.spin = new_spin;
		}

		//////////

		let new_ids = get_model_ids(self);

		for i in 0..self.update_status.len() {
			self.update_status[i] = original_ids[i] != new_ids[i];
		}

		Ok(())
	}
}

//////////

pub struct SpinitronState {
	continually_updated: ContinuallyUpdated<SpinitronStateData>
}

impl SpinitronState {
	pub fn new(api_key: &str) -> GenericResult<Self> {
		let data = SpinitronStateData::new(api_key)?;
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

	pub fn model_was_updated(&self, model_name: SpinitronModelName) -> bool {
		self.continually_updated.get_data().update_status[model_name as usize]
	}

	pub fn update(&mut self) -> GenericResult<bool> {
		self.continually_updated.update()
	}
}
