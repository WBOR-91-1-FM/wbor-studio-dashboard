use derive_alias::derive_alias;
use serde::{Serialize, Deserialize};

use crate::spinitron::wrapper_types::*;

// TODO: eventually, remove `Debug` from both of these
pub trait SpinitronModel: Serialize + for<'a> Deserialize<'a> + Clone + std::default::Default + std::fmt::Debug {
	fn get_id(&self) -> SpinitronModelId;
}

derive_alias! {derive_spinitron_model => #[derive(Serialize, Deserialize, Clone, Default, Debug)]}

// TODO: make these `impl`s less repetitive

// TODO: make a comparator instead, that compares the ids
impl SpinitronModel for Spin {fn get_id(&self) -> SpinitronModelId {self.id}}
impl SpinitronModel for Playlist {fn get_id(&self) -> SpinitronModelId {self.id}}
impl SpinitronModel for Persona {fn get_id(&self) -> SpinitronModelId {self.id}}
impl SpinitronModel for Show {fn get_id(&self) -> SpinitronModelId {self.id}}

impl Spin {
	pub fn get_playlist_id(&self) -> SpinitronModelId {self.playlist_id}
	pub fn get_image_link(&self) -> &MaybeString {&self.image}
}

impl Playlist {
	pub fn get_persona_id(&self) -> SpinitronModelId {self.persona_id}
	pub fn get_show_id(&self) -> Option<SpinitronModelId> {self.show_id}
	pub fn set_show_id(&mut self, id: SpinitronModelId) {self.show_id = Some(id);}
}

// TODO: for any `String` field, if it equals the empty string, set it to `None`

derive_spinitron_model!(
#[allow(dead_code)] // TODO: remove
pub struct Spin {
	// This does not cover all the spin fields; this is just the most useful subset of them.

	////////// These are officially enabled fields

	artist: String,
	local: MaybeBool,
	song: String,

	// TODO: why is `time` not there?

	duration: Uint,
	request: MaybeBool,
	new: MaybeBool,

	release: String,

	va: MaybeBool,

	medium: MaybeString, // This should just be `String`, but it isn't here, for some reason
	released: MaybeUint,

	////////// These are other fields

	// Ignoring "_links" for now. TODO: add start, end, and label later

	id: SpinitronModelId,
	playlist_id: SpinitronModelId,
	image: MaybeString // If there's no image, it will be `None` or `Some("")`
});

derive_spinitron_model!(
#[allow(dead_code)] // TODO: remove
pub struct Playlist {
	id: SpinitronModelId,
	persona_id: SpinitronModelId, // TODO: why are all the persona ids the same?
	show_id: MaybeSpinitronModelId, // TODO: why is this optional?

	start: String,
	end: String,
	duration: Uint,
	timezone: String,

	category: MaybeString,
	title: String,
	description: MaybeString,
	since: MaybeUint,

	url: MaybeString, // TODO: maybe remove this
	hide_dj: MaybeUint, // 0 or 1
	image: MaybeString,
	automation: MaybeUint, // 0 or 1

	episode_name: MaybeString,
	episode_description: MaybeString
});

derive_spinitron_model!(
#[allow(dead_code)] // TODO: remove
pub struct Persona {
	////////// These are fields that are officially supported by Spinitron

	id: SpinitronModelId,
	name: String,

	bio: MaybeString,
	since: MaybeUint,

	email: String, // If there's no email, it will be `""`
	website: MaybeString, // If there's no website, it will be `None` or `Some("")`
	image: MaybeString //  If there's no website, it will be `None`
});

derive_spinitron_model!(
#[allow(dead_code)] // TODO: remove
pub struct Show {
	id: SpinitronModelId,

	start: String,
	end: String,
	duration: Uint,
	timezone: String,

	one_off: Bool,

	category: String,
	title: String,
	description: String,

	since: MaybeUint,
	url: String,
	hide_dj: Uint, // 0 or 1
	image: MaybeString
});
