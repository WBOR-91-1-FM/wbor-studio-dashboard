use crate::{
	utility_types::{
		thread_task::ThreadTask,
		generic_result::{self, GenericResult, SendableGenericResult}
	},

	spinitron::{
		api_key::ApiKey,
		api::{get_current_spin, get_from_id},
		model::{SpinitronModel, SpinitronModelName, Spin, Playlist, Persona, Show}
	}
};

#[derive(Clone)]
struct SpinitronStateData {
	spin: Spin,
	playlist: Playlist,
	persona: Persona,
	show: Show, // TODO: will there ever not be a show?

	api_key: ApiKey,

	/* The boolean at index `i` is true if the model at index `i` was recently
	updated. Model indices are (in order) spin, playlist, persona, and show. */
	update_status: [bool; 4]
}

type SpinitronStateUpdateTask = ThreadTask<SendableGenericResult<SpinitronStateData>>;

pub struct SpinitronState {
	data: SpinitronStateData,
	curr_update_task: Option<SpinitronStateUpdateTask>
}

impl SpinitronStateData {
	fn new() -> GenericResult<Self> {
		let api_key = ApiKey::new()?;

		// TODO: if there is no current spin, will this only return the last one?
		let spin = get_current_spin(&api_key)?;
		let mut playlist: Playlist = get_from_id(&api_key, Some(spin.get_playlist_id()))?;
		let persona = get_from_id(&api_key, Some(playlist.get_persona_id()))?;
		let show = Self::get_show_while_syncing_show_id(&api_key, &mut playlist)?;

		/*
		let spin = Spin::default();
		let playlist = Playlist::default();
		let persona = Persona::default();
		let show = Show::default();
		*/

		Ok(Self {
			spin, playlist, persona, show, api_key,
			update_status: [false, false, false, false]
		})
	}

	fn get_show_while_syncing_show_id(api_key: &ApiKey, playlist: &mut Playlist) -> GenericResult<Show> {
		let show: Show = get_from_id(api_key, playlist.get_show_id())?;

		/* It's possible that the playlist will not have a show id
		(e.g. if someone plays songs, without making a playlist to log them).
		In that case, this gets the current show according to the schedule,
		and the playlist's show ID is set after that. */

		if playlist.get_show_id().is_none() {
			println!("The playlist's show id was None, so setting it manually to the show id");
			playlist.set_show_id(show.get_id());
		}

		Ok(show)
	}

	/* This returns a set of 4 booleans, indicating if the
	spin, playlist, persona, or show updated (in order). */
	fn update(&mut self) -> GenericResult<()> {
		let api_key = &self.api_key;
		let new_spin = get_current_spin(api_key)?;

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
			// Syncing the playlist

			let new_spin_playlist_id = new_spin.get_playlist_id();

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
						println!("Do conventional refresh for show id");
						self.show = get_from_id(api_key, Some(new_playlist_show_id))?;
					}
				}
				else {
					/* In this case, the playlist didn't have a show id. This means that
					someone is playing music without logging it to a show's playlist.
					From this, the current show will be inferred based on the schedule,
					and the current playlist's show will be synced with the one from that show. */
					println!("Do unconventional refresh for show id");
					self.show = Self::get_show_while_syncing_show_id(&api_key, &mut new_playlist)?;
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

impl SpinitronState {
	pub fn new() -> GenericResult<Self> {
		Ok(Self {
			data: SpinitronStateData::new()?,
			curr_update_task: None
		})
	}

	pub fn get_model_by_name(&self, name: SpinitronModelName) -> &dyn SpinitronModel {
		let data = &self.data;

		match name {
			SpinitronModelName::Spin => &data.spin,
			SpinitronModelName::Playlist => &data.playlist,
			SpinitronModelName::Persona => &data.persona,
			SpinitronModelName::Show => &data.show
		}
	}

	pub fn model_was_updated(&self, model_name: SpinitronModelName) -> bool {
		self.data.update_status[model_name as usize]
	}

	//////////

	fn make_new_update_task(&mut self) {
		let mut cloned_data = self.data.clone();

		let task = ThreadTask::new(
			move || {
				generic_result::make_sendable(cloned_data.update())?;
				Ok(cloned_data.clone())
			}
		);

		self.curr_update_task = Some(task);
	}

	pub fn update(&mut self) -> GenericResult<()> {
		match &self.curr_update_task {
			Some(task) => {
				if let Some(data) = task.get_value()? {
					self.data = data?;
					self.make_new_update_task();
				}
			},

			None => self.make_new_update_task()
		}

		Ok(())
	}
}
