use derive_alias::derive_alias;
use serde::{Serialize, Deserialize};

use crate::{
	spinitron::wrapper_types::*,
	texture::TextureCreationInfo
};

////////// This is a set of model-related traits

pub type MaybeTextureCreationInfo<'a> = Option<TextureCreationInfo<'a>>;

pub trait SpinitronModel {
	fn get_id(&self) -> SpinitronModelId;
	fn get_endpoint(&self) -> &'static str;
	fn get_texture_creation_info(&self) -> MaybeTextureCreationInfo;

	fn evaluate_image_url(url: &Option<String>) -> MaybeTextureCreationInfo where Self: Sized {
		if let Some(inner_url) = url {
			if !inner_url.is_empty() {
				return Some(TextureCreationInfo::Url(inner_url));
			}
		}
		None
	}
}

/* These properties are used for building spinitron models in `api.rs`.
They are not included by default because they do not allow the model to be object-safe. */
pub trait SpinitronModelWithProps:
	SpinitronModel + Clone + Default
	+ serde::Serialize + for<'de> serde::Deserialize<'de> {}

derive_alias! {derive_spinitron_model_props => #[derive(Serialize, Deserialize, Clone, Default)]}

////////// These are the implementations of the traits above

/* TODO:
- Make these `impl`s less repetitive
- Make a comparator instead that compares the ids
- Perhaps get the endpoint based on a model enum instead
*/

impl SpinitronModel for Spin {
	fn get_id(&self) -> SpinitronModelId {self.id}
	fn get_endpoint(&self) -> &'static str {"spins"}
	fn get_texture_creation_info(&self) -> MaybeTextureCreationInfo {Self::evaluate_image_url(&self.image)}
}

impl SpinitronModel for Playlist {
	fn get_id(&self) -> SpinitronModelId {self.id}
	fn get_endpoint(&self) -> &'static str {"playlists"}
	fn get_texture_creation_info(&self) -> MaybeTextureCreationInfo {Self::evaluate_image_url(&self.image)}
}

impl SpinitronModel for Persona {
	fn get_id(&self) -> SpinitronModelId {self.id}
	fn get_endpoint(&self) -> &'static str {"personas"}
	fn get_texture_creation_info(&self) -> MaybeTextureCreationInfo {Self::evaluate_image_url(&self.image)}
}

impl SpinitronModel for Show {
	fn get_id(&self) -> SpinitronModelId {self.id}
	fn get_endpoint(&self) -> &'static str {"shows"}
	fn get_texture_creation_info(&self) -> MaybeTextureCreationInfo {Self::evaluate_image_url(&self.image)}
}

impl Spin {
	pub fn get_playlist_id(&self) -> SpinitronModelId {self.playlist_id}
}

impl Playlist {
	pub fn get_persona_id(&self) -> SpinitronModelId {self.persona_id}
	pub fn get_show_id(&self) -> MaybeSpinitronModelId {self.show_id}
	pub fn set_show_id(&mut self, id: SpinitronModelId) {self.show_id = Some(id);}
}

impl SpinitronModelWithProps for Spin {}
impl SpinitronModelWithProps for Playlist {}
impl SpinitronModelWithProps for Persona {}
impl SpinitronModelWithProps for Show {}

////////// These are the model definitions

#[derive(Copy, Clone)]
pub enum SpinitronModelName {
	Spin, Playlist, Persona, Show
}

// TODO: for any `String` field, if it equals the empty string, set it to `None`

derive_spinitron_model_props!(
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

	// Ignoring "_links" for now. TODO: add start, end, and label later (given the start, can I figure out where I am in the song?)

	id: SpinitronModelId,
	playlist_id: SpinitronModelId,
	image: MaybeString // If there's no image, it will be `None` or `Some("")`
});

derive_spinitron_model_props!(
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

derive_spinitron_model_props!(
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

derive_spinitron_model_props!(
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
