// TODO: shorten these
use crate::spinitron::api_key::ApiKey;
use crate::spinitron::model::{Spin, Playlist, Persona, Show};
use crate::spinitron::api::{get_current_spin, get_playlist_from_id, get_persona_from_id, get_show_from_id};

use crate::generic_result::GenericResult;

pub struct SpinitronState {
	spin: Spin,
	playlist: Playlist,
	persona: Persona,
	show: Option<Show>,
	api_key: ApiKey
}

impl SpinitronState {
	pub fn new() -> GenericResult<Self> {
		let api_key = ApiKey::new()?;

		// TODO: if there is no current spin, will this only return the last one?
		let spin = get_current_spin(&api_key)?;
		let playlist = get_playlist_from_id(&api_key, spin.playlist_id)?;
		let persona = get_persona_from_id(&api_key, playlist.persona_id)?;
		let show = get_show_from_id(&api_key, playlist.show_id)?;

		/*
		let spin = Spin::default();
		let playlist = Playlist::default();
		let persona = Persona::default();
		let show = Some(Show::default());
		*/

		Ok(Self {spin, playlist, persona, show, api_key})
	}

	/* This returns a set of 4 booleans, indicating if the
	spin, playlist, persona, or show updated (in order). */
	pub fn update(&mut self) -> GenericResult<(bool, bool, bool, bool)> {
		let api_key = &self.api_key;
		let new_spin = get_current_spin(api_key)?;

		let original_ids = (
			self.spin.id,
			self.spin.playlist_id,
			self.playlist.persona_id,
			self.playlist.show_id
		);

		////////// TODO: make this less repetitive

		// Syncing the spin
		if self.spin.id != new_spin.id {
			// Syncing the playlist
			if self.spin.playlist_id != new_spin.playlist_id {
				let new_playlist = get_playlist_from_id(api_key, new_spin.playlist_id)?;

				// Syncing the persona
				if self.playlist.persona_id != new_playlist.persona_id {
					self.persona = get_persona_from_id(api_key, new_playlist.persona_id)?;
				}

				// Syncing the show
				if self.playlist.show_id != new_playlist.show_id {
					self.show = get_show_from_id(api_key, new_playlist.show_id)?;
				}

				self.playlist = new_playlist;
			}

			self.spin = new_spin;
		}

		Ok((
			original_ids.0 != self.spin.id,
			original_ids.1 != self.spin.playlist_id,
			original_ids.2 != self.playlist.persona_id,
			original_ids.3 != self.playlist.show_id
		))
	}
}
