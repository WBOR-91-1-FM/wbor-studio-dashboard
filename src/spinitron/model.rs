use std::borrow::Cow;

use regex::Regex;
use derive_alias::derive_alias;
use serde::{Serialize, Deserialize};

use crate::{
	spinitron::wrapper_types::*,
	texture::TextureCreationInfo
};

pub const NUM_SPINITRON_MODEL_TYPES: usize = 4;

// TODO: make these lazy-static regexps (find a matching lazy-static version from another package); or compile them at compile-time somehow
const SPIN_IMAGE_SIZE_REGEXP_PATTERN: &str = r#"\d+x\d+"#;
const SPIN_IMAGE_REGEXP_PATTERN: &str = r#"^https:\/\/.+\d+x\d+bb.jpg$"#;
const DEFAULT_PERSONA_AND_SHOW_IMAGE_REGEXP_PATTERN: &str = r#"^https:\/\/farm\d.staticflickr\.com\/\d+\/.+\..+$"#;

////////// This is a set of model-related traits

pub type MaybeTextureCreationInfo<'a> = Option<TextureCreationInfo<'a>>;

pub trait SpinitronModel {
	fn get_id(&self) -> SpinitronModelId;
	fn to_string(&self) -> String;
	fn get_texture_creation_info(&self, texture_size: (u32, u32)) -> MaybeTextureCreationInfo;

	fn evaluate_model_image_url<'a>(
		maybe_url: &'a Option<String>,
		inner_behavior: impl FnOnce(&'a str) -> MaybeTextureCreationInfo<'a>,
		make_fallback_for_no_url: impl FnOnce() -> MaybeTextureCreationInfo<'a>)

		-> MaybeTextureCreationInfo<'a> where Self: Sized {

		if let Some(url) = maybe_url {
			if !url.is_empty() {
				return inner_behavior(url);
			}
		}

		make_fallback_for_no_url()
	}

	fn evaluate_model_image_url_with_regexp<'a>(
		maybe_url: &'a Option<String>,
		make_fallback_for_no_url: impl FnOnce() -> MaybeTextureCreationInfo<'a>,

		regexp: &str,
		if_matches: impl FnOnce(&'a str) -> TextureCreationInfo<'a>,
		if_not: impl FnOnce(&'a str) -> TextureCreationInfo<'a>)

		-> MaybeTextureCreationInfo<'a> where Self: Sized {

		Self::evaluate_model_image_url(
			maybe_url,

			|url| {
				let compiled_regexp = Regex::new(regexp).unwrap();

				Some(if compiled_regexp.is_match(url) {if_matches(url)}
				else {if_not(url)})
			},

			make_fallback_for_no_url
		)
	}

	fn evaluate_model_image_url_for_persona_or_show<'a>(
		url: &'a Option<String>, image_for_no_persona_or_show: &'a str)

		-> MaybeTextureCreationInfo<'a> where Self: Sized {

		Self::evaluate_model_image_url_with_regexp(url,
			|| None,
			DEFAULT_PERSONA_AND_SHOW_IMAGE_REGEXP_PATTERN,

			// If it matches the default pattern, use the no-persona or no-show image
			|_| TextureCreationInfo::Path(Cow::Borrowed(image_for_no_persona_or_show)),

			// If it doesn't match the default pattern, use the provided image
			|url| TextureCreationInfo::Url(Cow::Borrowed(url))
		)
	}
}

/* These properties are used for building Spinitron models in `api.rs`.
They are not included by default because they do not allow the model to be object-safe. */
pub trait SpinitronModelWithProps:
	SpinitronModel + Clone + Default
	+ serde::Serialize + for<'de> serde::Deserialize<'de> {}

derive_alias! {derive_spinitron_model_props => #[derive(Serialize, Deserialize, Clone, Default)]}

////////// These are the implementations of the traits above

/* TODO:
- Make these `impl`s less repetitive (use a macro?)
- Make a comparator instead that compares the ids
*/

impl SpinitronModel for Spin {
	fn get_id(&self) -> SpinitronModelId {self.id}
	fn to_string(&self) -> String {format!("{} (from {}), by {}.", self.song, self.release, self.artist)}

	fn get_texture_creation_info(&self, (texture_width, texture_height): (u32, u32)) -> MaybeTextureCreationInfo {
		Self::evaluate_model_image_url_with_regexp(&self.image,
			|| None,
			SPIN_IMAGE_REGEXP_PATTERN,

			|url| {
				let size_pattern = Regex::new(SPIN_IMAGE_SIZE_REGEXP_PATTERN).unwrap();
				let with_size = size_pattern.replace(url, format!("{texture_width}x{texture_height}"));
				TextureCreationInfo::Url(with_size)
			},

			|url| {
				println!("The core structure of the spin image URL has changed. Failing URL: '{url}'. Unclear how to modify spin image size now.");
				TextureCreationInfo::Url(Cow::Borrowed(url))
			}
		)
	}
}

impl SpinitronModel for Playlist {
	fn get_id(&self) -> SpinitronModelId {self.id}
	fn to_string(&self) -> String {format!("Playlist: {}", self.title)}

	fn get_texture_creation_info(&self, _: (u32, u32)) -> MaybeTextureCreationInfo {
		Self::evaluate_model_image_url(&self.image, |url| Some(TextureCreationInfo::Url(Cow::Borrowed(url))), || None)
	}
}

impl SpinitronModel for Persona {
	fn get_id(&self) -> SpinitronModelId {self.id}
	fn to_string(&self) -> String {format!("Welcome, {}!", self.name)}

	fn get_texture_creation_info(&self, _: (u32, u32)) -> MaybeTextureCreationInfo {
		Self::evaluate_model_image_url_for_persona_or_show(&self.image, "assets/wbor_no_persona_image.png")
	}
}

impl SpinitronModel for Show {
	fn get_id(&self) -> SpinitronModelId {self.id}
	fn to_string(&self) -> String {format!("This is '{}'.", self.title)}

	fn get_texture_creation_info(&self, _: (u32, u32)) -> MaybeTextureCreationInfo {
		Self::evaluate_model_image_url_for_persona_or_show(&self.image, "assets/wbor_no_show_image.png")
	}
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
