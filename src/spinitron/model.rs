use std::borrow::Cow;
use std::collections::HashMap;

use regex::Regex;
use derive_alias::derive_alias;
use serde::{Serialize, Deserialize};

use crate::{
	texture::TextureCreationInfo,
	utility_types::generic_result::*,

	spinitron::{
		wrapper_types::*,
		api::get_model_from_id
	}
};

pub const NUM_SPINITRON_MODEL_TYPES: usize = 4;

lazy_static::lazy_static!(
	static ref SPIN_IMAGE_SIZE_REGEXP: Regex = Regex::new(r#"\d+x\d+bb"#).unwrap();
	static ref SPIN_IMAGE_REGEXP: Regex = Regex::new(r#"^https:\/\/.+\d+x\d+bb.+$"#).unwrap();
	static ref DEFAULT_PERSONA_AND_SHOW_IMAGE_REGEXP: Regex = Regex::new(r#"^https:\/\/farm\d.staticflickr\.com\/\d+\/.+\..+$"#).unwrap();

	static ref PLAYLIST_CATEGORY_EMOJIS_MAPPING: HashMap<&'static str, &'static str> = HashMap::from([
		("Automation", "🤖"),
		("Ambient", "🌌"),
		("Blues", "🎺"),
		("Classical", "🎼🎻"),
		("Country", "🤠👢"),
		("Dance", "🕺🪩💃"),
		("Electronic", "⚡"),
		("Experimental", "🌀"),
		("Folk", "🪕"),
		("Hip-Hop", "📾"),
		("Jazz", "🎷"),
		("Metal", "🤘👹"),
		("Music", "🎵"),
		("News", "📰"),
		("Pop", "🎤"),
		("Punk", "🤘🔥"),
		("RnB", "﹏🎺﹏"),
		("Regional", "🗺️"),
		("Rock", "🎸"),
		("Talk", "🗣️")
	]);
);

////////// This is a set of model-related traits

pub type MaybeTextureCreationInfo<'a> = Option<TextureCreationInfo<'a>>;

pub trait SpinitronModel {
	fn get_id(&self) -> SpinitronModelId;
	fn get_end_time(&self) -> GenericResult<chrono::DateTime<chrono::Utc>>;

	fn to_string(&self, expired: bool) -> Cow<str>;
	fn get_texture_creation_info(&self, texture_size: (u32, u32)) -> MaybeTextureCreationInfo;
	fn get_texture_creation_info_when_expired(&self) -> TextureCreationInfo;

	fn parse_time(time: &str) -> GenericResult<chrono::DateTime<chrono::Utc>> where Self: Sized {
		let mut amended_end = time.to_owned();
		amended_end.insert(amended_end.len() - 2, ':');
		Ok(chrono::DateTime::parse_from_rfc3339(&amended_end)?.into())
	}

	fn evaluate_model_image_url<'a>(
		maybe_url: &'a Option<String>,
		inner_behavior: impl FnOnce(&'a str) -> MaybeTextureCreationInfo<'a>,
		make_fallback_for_no_url: impl FnOnce() -> MaybeTextureCreationInfo<'a>)

		-> MaybeTextureCreationInfo where Self: Sized {

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

		regexp: &Regex,
		if_matches: impl FnOnce(&'a str) -> TextureCreationInfo<'a>,
		if_not: impl FnOnce(&'a str) -> TextureCreationInfo<'a>)

		-> MaybeTextureCreationInfo<'a> where Self: Sized {

		Self::evaluate_model_image_url(
			maybe_url,

			|url| {
				Some(
					if regexp.is_match(url) {if_matches(url)}
					else {if_not(url)}
				)
			},

			make_fallback_for_no_url
		)
	}

	fn evaluate_model_image_url_for_persona_or_show<'a>(
		url: &'a Option<String>, image_for_no_persona_or_show: &'a str)

		-> MaybeTextureCreationInfo<'a> where Self: Sized {

		let fallback = TextureCreationInfo::Path(Cow::Borrowed(image_for_no_persona_or_show));

		Self::evaluate_model_image_url_with_regexp(url,
			|| Some(fallback.clone()),
			&DEFAULT_PERSONA_AND_SHOW_IMAGE_REGEXP,

			// If it matches the default pattern, use the no-persona or no-show image
			|_| fallback.clone(),

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
	fn get_end_time(&self) -> GenericResult<chrono::DateTime<chrono::Utc>> {Self::parse_time(&self.end)}

	// TODO: for this, can I split it up into multiple lines, and then render multiline text somehow?
	fn to_string(&self, expired: bool) -> Cow<str> {
		if expired {
			Cow::Borrowed("No 😰 recent 😬 spins 😟❗")
		}
		else {
			Cow::Owned(format!("{} (from {}), by {}", self.song, self.release, self.artist))
		}
	}

	fn get_texture_creation_info(&self, (texture_width, texture_height): (u32, u32)) -> MaybeTextureCreationInfo {
		Self::evaluate_model_image_url_with_regexp(&self.image,
			|| None,
			&SPIN_IMAGE_REGEXP,

			|url| {
				let with_size = SPIN_IMAGE_SIZE_REGEXP.replace(url, format!("{texture_width}x{texture_height}bb"));
				TextureCreationInfo::Url(with_size)
			},

			|url| {
				log::error!("The core structure of the spin image URL has changed. Failing URL: '{url}'. Unclear how to modify spin image size now.");
				TextureCreationInfo::Url(Cow::Borrowed(url))
			}
		)
	}

	fn get_texture_creation_info_when_expired(&self) -> TextureCreationInfo {
		TextureCreationInfo::Path(Cow::Borrowed("assets/polar_headphones_logo.png"))
	}
}

impl SpinitronModel for Playlist {
	fn get_id(&self) -> SpinitronModelId {self.id}
	fn get_end_time(&self) -> GenericResult<chrono::DateTime<chrono::Utc>> {Self::parse_time(&self.end)}

	fn to_string(&self, expired: bool) -> Cow<str> {
		if expired {
			Cow::Borrowed("Get ready, the next show is coming soon...")
		}
		else {
			let (mut show_emojis, mut spacing) = ("", "");

			// If there's no category, it's probably an automation playlist
			if let Some(category) = &self.category {
				if let Some(emojis) = PLAYLIST_CATEGORY_EMOJIS_MAPPING.get(category.as_str()) {
					show_emojis = emojis;
					spacing = " ";
				}
				else {
					log::warn!("Unrecognized genre '{category}' for playlist with name '{}'", self.title);
				}
			}

			Cow::Owned(format!("{show_emojis}{spacing}This is '{}'{spacing}{show_emojis}", self.title))
		}
	}

	fn get_texture_creation_info(&self, _: (u32, u32)) -> MaybeTextureCreationInfo {
		Self::evaluate_model_image_url(&self.image, |url| Some(TextureCreationInfo::Url(Cow::Borrowed(url))), || None)
	}

	// This function is not used at the moment
	fn get_texture_creation_info_when_expired(&self) -> TextureCreationInfo {
		TextureCreationInfo::Path(Cow::Borrowed("assets/after_show_image.png"))
	}

}

impl SpinitronModel for Persona {
	fn get_id(&self) -> SpinitronModelId {self.id}
	fn get_end_time(&self) -> GenericResult<chrono::DateTime<chrono::Utc>> {Ok(chrono::DateTime::<chrono::Utc>::MAX_UTC)}

	fn to_string(&self, expired: bool) -> Cow<str> {
		if expired {
			Cow::Borrowed("You have reached the end of time!")
		}
		else {
			Cow::Owned(format!("Welcome, {}!", self.name))
		}
	}

	fn get_texture_creation_info(&self, _: (u32, u32)) -> MaybeTextureCreationInfo {
		Self::evaluate_model_image_url_for_persona_or_show(&self.image, "assets/no_persona_image.png")
	}

	// This is not ever called at the moment
	fn get_texture_creation_info_when_expired(&self) -> TextureCreationInfo {
		TextureCreationInfo::Path(Cow::Borrowed("assets/polar_headphones_logo.png"))
	}
}

impl SpinitronModel for Show {
	fn get_id(&self) -> SpinitronModelId {self.id}
    fn get_end_time(&self) -> GenericResult<chrono::DateTime<chrono::Utc>> {Self::parse_time(&self.end)}

	fn to_string(&self, expired: bool) -> Cow<str> {
		let tense = if expired {"was"} else {"is"};
		Cow::Owned(format!("This {tense} '{}'", self.title))
	}

	/* TODO: will this update on time after pre-show no-show-images?
	And are playlist images the same as show images; and if so,
	can I just use them instead? And could it happen that this sometimes
	shows past shows too?  It might be nice to generalize this with some
	type of `ModelAgeState` enum. */
	fn get_texture_creation_info(&self, _: (u32, u32)) -> MaybeTextureCreationInfo {
		let no_show_image_path = "assets/no_show_image.png";

		// This logic is here to avoid displaying show images before their start times
		if chrono::Utc::now() < Self::parse_time(&self.start).unwrap() { // TODO: don't unwrap
			return Some(TextureCreationInfo::Path(Cow::Borrowed(no_show_image_path)))
		}
		else {
			Self::evaluate_model_image_url_for_persona_or_show(&self.image, no_show_image_path)
		}
	}

	fn get_texture_creation_info_when_expired(&self) -> TextureCreationInfo {
		TextureCreationInfo::Path(Cow::Borrowed("assets/after_show_image.jpg"))
	}
}

impl Spin {
	// TODO: can I reduce the repetition on the `get`s?
	pub fn get(api_key: &str) -> GenericResult<Self> {get_model_from_id(api_key, None)}
}

impl Playlist {
	pub fn get(api_key: &str) -> GenericResult<Self> {get_model_from_id(api_key, None)}
}

impl Persona {
	pub fn get(api_key: &str, playlist: &Playlist) -> GenericResult<Self> {
		get_model_from_id(api_key, Some(playlist.persona_id))
	}
}

impl Show {
	pub fn get(api_key: &str) -> GenericResult<Self> {
		/* I could make an optimization that only checks this on 0-minute and 30-minute marks,
		but playlists may be made a few minutes after a start time, so this would not cover those. */
		get_model_from_id(api_key, None)
	}
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
	end: String,

	request: MaybeBool,
	new: MaybeBool,

	release: String,

	va: MaybeBool,

	medium: MaybeString, // This should just be `String`, but it isn't here, for some reason
	released: MaybeUint,

	////////// These are other fields

	/*
	- Ignoring "_links" for now.
	- Also not  keeping the playlist ID here, since if someone doesn't come to their show, then the playlist ID will be invalid.
	- TODO: add start, end, and label later (given the start, can I figure out where I am in the song?)
	*/

	id: SpinitronModelId,
	image: MaybeString // If there's no image, it will be `None` or `Some("")`
});

derive_spinitron_model_props!(
#[allow(dead_code)] // TODO: remove
pub struct Playlist {
	id: SpinitronModelId,
	persona_id: SpinitronModelId, // TODO: why are all the persona ids the same?

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
	id: SpinitronModelId, // Note: some shows will have the same IDS, but different times (e.g. WBOR's Commodore 64)

	start: String,
	end: String,
	duration: Uint,
	timezone: String,

	one_off: Bool,

	category: MaybeString, // This will always be set, in practice (why did I make it `MaybeString`?)
	title: String, // The titles will generally never be empty
	description: String, // This will sometimes be empty (HTML-formatted)

	since: MaybeUint,
	url: String,
	hide_dj: Uint, // 0 or 1
	image: MaybeString
});
