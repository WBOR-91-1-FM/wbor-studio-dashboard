use std::{
	sync::Arc,
	borrow::Cow,
	collections::HashMap
};

use regex::Regex;
use derive_alias::derive_alias;
use serde::{Serialize, Deserialize};

use crate::{
	window_tree::PixelAreaSDL,
	texture::pool::TextureCreationInfo,

	utils::{
		time::*,
		generic_result::*
	},

	spinitron::{
		wrapper_types::*,
		state::ModelAgeState,
		api::{get_model_from_id, get_most_recent_model, get_models}
	}
};

////////// These are some constants:

pub const NUM_SPINITRON_MODEL_TYPES: usize = 4;

// TODO: switch to `once_cell` at some point (and in other places too)
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
	fn get_time_range(&self) -> (ReferenceTimestamp, Option<ReferenceTimestamp>);

	fn to_string(&self, age_state: ModelAgeState) -> Cow<'static, str>;
	fn get_texture_creation_info(&self, age_state: ModelAgeState, spin_texture_window_size: PixelAreaSDL) -> MaybeTextureCreationInfo;

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

		let make_fallback = || TextureCreationInfo::from_path(image_for_no_persona_or_show);

		Self::evaluate_model_image_url_with_regexp(url,
			|| Some(make_fallback()),
			&DEFAULT_PERSONA_AND_SHOW_IMAGE_REGEXP,

			// If it matches the default pattern, use the no-persona or no-show image
			|_| make_fallback(),

			// If it doesn't match the default pattern, use the provided image
			|url| TextureCreationInfo::Url(Cow::Borrowed(url))
		)
	}
}

//////////

#[derive(Copy, Clone, PartialEq)]
pub enum SpinitronModelName {
	Spin, Playlist, Persona, Show
}

/* These properties are used for building Spinitron models in `api.rs`.
They are not included by default because they do not allow the model to be object-safe. */
pub trait SpinitronModelWithProps:
	SpinitronModel + Clone + Default
	+ Serialize + for<'de> Deserialize<'de> {}

// TODO: reduce the repetition here
impl SpinitronModelWithProps for Spin {}
impl SpinitronModelWithProps for Playlist {}
impl SpinitronModelWithProps for Persona {}
impl SpinitronModelWithProps for Show {}

derive_alias! {derive_spinitron_model_props => #[derive(Serialize, Deserialize, Clone, Default)]}

//////////

impl Spin {
	pub async fn get_current_and_history(api_key: &str, history_amount: usize) -> GenericResult<(Arc<Self>, Vec<Self>)> {
		// Getting 1 more than the history amount, since we need the current spin too
		let mut models = get_models(api_key, Some(history_amount + 1)).await?;
		let first = Arc::new(models.remove(0));
		Ok((first, models))
	}

	pub fn get_start_time(&self) -> ReferenceTimestamp {
		self.start
	}
}

impl Playlist {
	pub async fn get(api_key: &str) -> GenericResult<Arc<Self>> {
		Ok(Arc::new(get_most_recent_model(api_key).await?))
	}
}

impl Persona {
	pub async fn get(api_key: &str, playlist: &Playlist) -> GenericResult<Arc<Self>> {
		let mut persona: Persona = get_model_from_id(api_key, playlist.persona_id).await?;

		// Copy over the playlist start/end, since a persona should have one too (it doesn't have one given to it by Spinitron)
		(persona.start_from_associated_playlist, persona.end_from_associated_playlist) = (playlist.start, playlist.end);

		Ok(Arc::new(persona))
	}
}

impl Show {
	pub async fn get(api_key: &str) -> GenericResult<Arc<Self>> {
		Ok(Arc::new(get_most_recent_model(api_key).await?))
	}
}

////////// These are the implementations of the traits above

/* TODO:
- Make these `impl`s less repetitive (use a macro?)
- Make a comparator somehow that compares the ids?
*/

impl SpinitronModel for Spin {
	fn get_id(&self) -> SpinitronModelId {self.id}
	fn get_time_range(&self) -> (ReferenceTimestamp, Option<ReferenceTimestamp>) {(self.start, self.end)}

	// TODO: for this, can I split the outut string into multiple lines, and then render multiline text somehow?
	fn to_string(&self, age_state: ModelAgeState) -> Cow<'static, str> {
		match age_state {
			ModelAgeState::BeforeIt =>
				Cow::Borrowed("Are you a time traveler or something???"),
			ModelAgeState::CurrentlyActive | ModelAgeState::AfterIt =>
				Cow::Owned(format!("{} (from {}), by {}", self.song, self.release, self.artist)),
			ModelAgeState::AfterItFromCustomExpiryDuration =>
				Cow::Borrowed("No 😰 recent 😬 spins 😟❗")
		}
	}

	fn get_texture_creation_info(&self, age_state: ModelAgeState, (texture_width, texture_height): PixelAreaSDL) -> MaybeTextureCreationInfo {
		if age_state == ModelAgeState::AfterItFromCustomExpiryDuration {
			Some(TextureCreationInfo::from_path("assets/polar_headphones_logo.png"))
		}
		else {
			Self::evaluate_model_image_url_with_regexp(&self.image,
				|| None,
				&SPIN_IMAGE_REGEXP,

				|url| {
					// TODO: figure out if there's a good reason why this URL fails often
					let with_size = SPIN_IMAGE_SIZE_REGEXP.replace(url, format!("{texture_width}x{texture_height}bb"));
					TextureCreationInfo::Url(with_size)
				},

				|url| {
					log::error!("The core structure of the spin image URL has changed. Failing URL: '{url}'. Unclear how to modify spin image size now.");
					TextureCreationInfo::Url(Cow::Borrowed(url))
				}
			)
		}
	}
}

//////////

impl SpinitronModel for Playlist {
	fn get_id(&self) -> SpinitronModelId {self.id}
	fn get_time_range(&self) -> (ReferenceTimestamp, Option<ReferenceTimestamp>) {(self.start, Some(self.end))}

	fn to_string(&self, age_state: ModelAgeState) -> Cow<'static, str> {
		match age_state {
			ModelAgeState::BeforeIt =>
				Cow::Borrowed("How are you before a playlist that hasn't even started yet?"),

			ModelAgeState::CurrentlyActive => {
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

			ModelAgeState::AfterIt => Cow::Borrowed("Make a playlist, if you're there!"),

			// Note: the custom expiry duration is expected to be negative here
			ModelAgeState::AfterItFromCustomExpiryDuration => {
				Cow::Borrowed(if self.automation == Some(1) {
					"This automation playlist is coming to an end..."
				}
				else {
					"Pack up, and log out of Spinitron! The next show is starting soon."
				})
			}
		}
	}

	fn get_texture_creation_info(&self, age_state: ModelAgeState, _: PixelAreaSDL) -> MaybeTextureCreationInfo {
		match age_state {
			ModelAgeState::BeforeIt =>
				// TODO: is this even possible? If not, remove the associated image, perhaps...
				Some(TextureCreationInfo::from_path("assets/before_show_image.jpg")),

			ModelAgeState::CurrentlyActive | ModelAgeState::AfterItFromCustomExpiryDuration => {
				if self.automation == Some(1) {
					Some(TextureCreationInfo::from_path("assets/automation_playlist.png"))
				}
				else {
					Self::evaluate_model_image_url_for_persona_or_show(&self.image, "assets/no_show_image.png")
				}
			}

			ModelAgeState::AfterIt =>
				Some(TextureCreationInfo::from_path("assets/after_show_image.jpg"))
		}
	}
}

//////////

impl SpinitronModel for Persona {
	fn get_id(&self) -> SpinitronModelId {self.id}

	fn get_time_range(&self) -> (ReferenceTimestamp, Option<ReferenceTimestamp>) {
		(self.start_from_associated_playlist, Some(self.end_from_associated_playlist))
	}

	fn to_string(&self, age_state: ModelAgeState) -> Cow<'static, str> {
		if age_state == ModelAgeState::AfterItFromCustomExpiryDuration {
			Cow::Borrowed("No one is expected in the studio right now...")
		}
		else {
			Cow::Owned(format!("Welcome, {}!", self.name))
		}
	}

	fn get_texture_creation_info(&self, age_state: ModelAgeState, _: PixelAreaSDL) -> MaybeTextureCreationInfo {
		if age_state == ModelAgeState::AfterItFromCustomExpiryDuration {
			Some(TextureCreationInfo::from_path("assets/no_person_in_studio.jpg"))
		}
		else {
			Self::evaluate_model_image_url_for_persona_or_show(&self.image, "assets/no_persona_image.png")
		}
	}
}

//////////

impl SpinitronModel for Show {
	fn get_id(&self) -> SpinitronModelId {self.id}
	fn get_time_range(&self) -> (ReferenceTimestamp, Option<ReferenceTimestamp>) {(self.start, Some(self.end))}

	// This function is not used at the moment
	fn to_string(&self, _: ModelAgeState) -> Cow<'static, str> {Cow::Borrowed("")}

	// This function is not used at the moment
	fn get_texture_creation_info(&self, _: ModelAgeState, _: PixelAreaSDL) -> MaybeTextureCreationInfo {None}
}

////////// These are the model definitions

// TODO: for any `String` field, if it equals the empty string, set it to `None`
// Note: the commented-out fields in each model below can be used, but have been removed for now (just not currently used).

fn report_spin_no_end_situation() -> Option<ReferenceTimestamp> {
	// The other 2 situations are logged in `serde_parse::maybe_spinitron_timestamp`
	log::error!("Rare spin end-time situation #1: the field is just not present.");
	None
}

derive_spinitron_model_props!(
pub struct Spin {
	id: SpinitronModelId,

	#[serde(deserialize_with = "serde_parse::spinitron_timestamp")]
	start: ReferenceTimestamp,
	#[serde(deserialize_with = "serde_parse::maybe_spinitron_timestamp", default = "report_spin_no_end_situation")]
	end: Option<ReferenceTimestamp>,

	artist: String,
	song: String,

	release: String,
	image: MaybeString // If there's no image, it will be `None` or `Some("")`

	// local: MaybeBool,
	// duration: MaybeUint, // This, along with the `end` field, are very rarely `None`
	// request: MaybeBool,
	// new: MaybeBool,
	// va: MaybeBool,
	// medium: MaybeString, // This should just be `String`, but it isn't here, for some reason
	// released: MaybeUint
});

derive_spinitron_model_props!(
pub struct Playlist {
	id: SpinitronModelId,
	persona_id: SpinitronModelId,

	#[serde(deserialize_with = "serde_parse::spinitron_timestamp")]
	start: ReferenceTimestamp,
	#[serde(deserialize_with = "serde_parse::spinitron_timestamp")]
	end: ReferenceTimestamp,

	title: String,
	image: MaybeString,
	category: MaybeString,
	automation: MaybeUint // 0 or 1

	// duration: Uint,
	// timezone: String,
	// description: MaybeString,
	// since: MaybeUint,
	// url: MaybeString, // TODO: maybe remove this
	// hide_dj: MaybeUint, // 0 or 1
	// episode_name: MaybeString,
	// episode_description: MaybeString
});

derive_spinitron_model_props!(
pub struct Persona {
	////////// These are fields that are officially supported by Spinitron

	id: SpinitronModelId,
	name: String,
	image: MaybeString, // If there's no image, it will be `None`

	// bio: MaybeString,
	// since: MaybeUint,
	// email: String, // If there's no email, it will be `""`
	// website: MaybeString, // If there's no website, it will be `None` or `Some("")`

	////////// These are the fields that I added after the fact (not associated with Spinitron's API)

	#[serde(skip_serializing, skip_deserializing)]
	start_from_associated_playlist: ReferenceTimestamp,
	#[serde(skip_serializing, skip_deserializing)]
	end_from_associated_playlist: ReferenceTimestamp
});

derive_spinitron_model_props!(
pub struct Show {
	id: SpinitronModelId, // Note: some shows will have the same IDs, but different times (e.g. "WBOR's Commodore 64")

	#[serde(deserialize_with = "serde_parse::spinitron_timestamp")]
	start: ReferenceTimestamp,
	#[serde(deserialize_with = "serde_parse::spinitron_timestamp")]
	end: ReferenceTimestamp

	// duration: Uint,
	// timezone: String,
	// one_off: Bool,
	// category: MaybeString, // This will always be set, in practice (TODO: why did I make it `MaybeString`?)
	// title: String, // The titles will generally never be empty
	// description: String, // This will sometimes be empty (HTML-formatted)
	// since: MaybeUint,
	// url: String,
	// hide_dj: Uint, // 0 or 1
	// image: MaybeString
});
