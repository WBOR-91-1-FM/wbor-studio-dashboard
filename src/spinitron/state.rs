use crate::{
	utility_types::generic_result::GenericResult,

	spinitron::{
		api_key::ApiKey,
		api::{get_current_spin, get_from_id},
		model::{SpinitronModel, Spin, Playlist, Persona, Show}
	}
};

pub struct SpinitronState {
	spin: Spin,
	playlist: Playlist,
	persona: Persona,
	show: Show, // TODO: will there ever not be a show?
	api_key: ApiKey
}

impl SpinitronState {
	// TODO: use a macro for this
	pub fn get_spin(&self) -> &Spin {&self.spin}
	pub fn get_playlist(&self) -> &Playlist {&self.playlist}
	pub fn get_persona(&self) -> &Persona {&self.persona}
	pub fn get_show(&self) -> &Show {&self.show}

	fn get_show_while_syncing_show_id(api_key: &ApiKey, playlist: &mut Playlist) -> GenericResult<Show> {
		let show: Show = get_from_id(&api_key, playlist.get_show_id())?;

		/* It's possible that the playlist will not have a show id
		(e.g. if someone plays songs, without making a playlist to log them).
		In that case, this gets the current show according to the schedule,
		and the playlist's show ID is set after that. */

		if let None = playlist.get_show_id() {
			println!("The playlist's show id was None, so setting it manually to the show id");
			playlist.set_show_id(show.get_id());
		}

		Ok(show)
	}

	pub fn new() -> GenericResult<Self> {
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

		Ok(Self {spin, playlist, persona, show, api_key})
	}

	/* This returns a set of 4 booleans, indicating if the
	spin, playlist, persona, or show updated (in order). */
	pub fn update(&mut self) -> GenericResult<(bool, bool, bool, bool)> {
		let api_key = &self.api_key;
		let new_spin = get_current_spin(api_key)?;

		// TODO: do a loop to get the ids instead
		let original_ids = (
			self.spin.get_id(),
			self.playlist.get_id(),
			self.persona.get_id(),
			self.show.get_id()
		);

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

				//////////

				self.playlist = new_playlist;
			}

			self.spin = new_spin;
		}

		Ok((
			original_ids.0 != self.spin.get_id(),
			original_ids.1 != self.playlist.get_id(),
			original_ids.2 != self.persona.get_id(),
			original_ids.3 != self.show.get_id()
		))
	}
}
