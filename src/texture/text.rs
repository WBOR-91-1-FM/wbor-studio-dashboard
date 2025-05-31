use sdl2::ttf;

use std::{
	borrow::Cow,
	collections::HashMap
};

use crate::{
	texture::pool,
	window_tree::{ColorSDL, PixelAreaSDL}
};

////////// Defining some hashable wrapper types

macro_rules! define_hashable_wrapper {
	($name:ident, $inner:ty, $hash_fn:expr) => {
		#[derive(Clone, Debug)]
		pub struct $name {
			field: $inner
		}

		impl std::hash::Hash for $name {
			fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
				let typed_hash_fn: fn(&$inner, &mut H) = $hash_fn;
				(typed_hash_fn)(&self.field, state);
			}
		}

		impl std::ops::Deref for $name {
			type Target = $inner;

			fn deref(&self) -> &Self::Target {
				&self.field
			}
		}
	};
}

define_hashable_wrapper!(HashableF64, f64, |inner, state| inner.to_bits().hash(state));
define_hashable_wrapper!(HashableHinting, ttf::Hinting, |inner, state| (inner.clone() as i32).hash(state));

define_hashable_wrapper!(HashableTextTextureScrollEaser, TextTextureScrollEaser, |inner, state| {
	inner.0.hash(state);
	inner.1.to_bits().hash(state);
});

////////// Defining `FontInfo`

#[derive(Clone, Hash, Debug)]
pub struct FontInfo {
	/* TODO:
	- Support non-static bytes for these two
	- Allow for a variable number of fallback fonts too
	- Only load fallbacks when necessary
	- Check if the entire Unicode supplementary multilingual plane is supported
	*/

	pub(in crate::texture) bytes: &'static [u8],
	pub(in crate::texture) unusual_chars_fallback_bytes: &'static [u8],

	pub(in crate::texture) font_has_char: fn(&ttf::Font, char) -> bool,

	pub(in crate::texture) style: ttf::FontStyle,
	pub(in crate::texture) hinting: HashableHinting,
	pub(in crate::texture) maybe_outline_width: Option<u16>
}

impl FontInfo {
	pub const fn new(bytes: &'static [u8], unusual_chars_fallback_bytes: &'static [u8],
		font_has_char: fn(&ttf::Font, char) -> bool, style: ttf::FontStyle,
		hinting: ttf::Hinting, maybe_outline_width: Option<u16>) -> Self {

		Self {
			bytes,
			unusual_chars_fallback_bytes,

			font_has_char,
			style,
			hinting: HashableHinting {field: hinting},
			maybe_outline_width
		}
	}

	pub fn with_style(&self, style: ttf::FontStyle) -> Self {
		let mut cloned = self.clone();
		cloned.style = style;
		cloned
	}
}

////////// Defining `DisplayText`

#[derive(Clone, Hash, Debug)]
pub struct DisplayText<'a> {
	text: Cow<'a, str>
}

impl DisplayText<'_> {
	pub fn new(text: &str) -> Self {
		// Indicates that emojis should be made colored; not rendered correctly on the Pi
		const UNICODE_VARIATION_SELECTOR_16: char = '\u{FE0F}';

		const WHITESPACE_REPLACEMENT_PAIRS: [(char, &str); 4] = [
			('\t', "    "),
			('\n', " "),
			('\r', " "),
			(UNICODE_VARIATION_SELECTOR_16, "")
		];

		/* TODO:
		- Should I add the rest of the blank characters (see https://invisible-characters.com/ for all), for better cleanup?
		- The second reason for this is to stop 'nonavailable' character variants to appear - although this would be hard to verify
		- Does character 157 have to be handled? It might crash the dashboard...
		*/
		const ALL_WHITESPACE_CHARS: [char; 5] = [
			' ', '\t', '\n', '\r', UNICODE_VARIATION_SELECTOR_16
		];

		//////////

		let trimmed_text = text.trim();
		let is_whitespace = |c: char| ALL_WHITESPACE_CHARS.contains(&c);

		/* If a string is only whitespace, make it empty.
		This also implicitly covers completely empty strings,
		and plenty of blank Unicode characters (that comes from `trim`).

		Note that this does not return "<BLANK TEXT>" since the case for that
		is based on if the rendered surface has zero width, not based on the contained
		characters for the string (and the former should be more reliable). */
		if trimmed_text.chars().all(is_whitespace) {
			return Self {text: Cow::Borrowed("")};
		}

		////////// Replacing all replacable whitespace chars with a single space

		// TODO: can I do this more efficiently (e.g. with regexps)?
		let mut adjusted = trimmed_text.to_owned();

		for (from, to) in WHITESPACE_REPLACEMENT_PAIRS {
			if adjusted.contains(from) {
				adjusted = adjusted.replace(from, to);
			}
		}

		////////// Returning

		Self {text: Cow::Owned(adjusted)}
	}

	// This assumes that the inputted padding characters should not be trimmed/preprocessed at all
	pub fn with_padding(self, left: &str, right: &str) -> Self {
		let mut text = self.text.to_string();
		text.insert_str(0, left);
		text.push_str(right);
		Self {text: text.into()}
	}

	pub fn inner(&self) -> &str {
		&self.text
	}
}

////////// Defining `TextTextureScrollEaser`, and `TextDisplayInfo`

/*
The first item, the function itself:
	Input: seed (some number of real-time fractional seconds), the period of the function, and if the text fits fully in the box.
	Output: scroll amount (range: [0, 1]), and if the text should wrap or not.
The second item is the period.
*/
pub type TextTextureScrollEaser = (fn(f64, f64, bool) -> (f64, bool), f64);

#[derive(Clone, Hash, Debug)]
pub struct TextDisplayInfo<'a> {
	pub(in crate::texture) text: DisplayText<'a>,
	pub(in crate::texture) color: ColorSDL,
	pub(in crate::texture) pixel_area: PixelAreaSDL,
	pub(in crate::texture) scroll_easer: HashableTextTextureScrollEaser,
	pub(in crate::texture) scroll_speed_multiplier: HashableF64
}

impl<'a> TextDisplayInfo<'a> {
	pub const fn new(display_text: DisplayText<'a>, color: ColorSDL,
		pixel_area: PixelAreaSDL, scroll_easer: TextTextureScrollEaser,
		scroll_speed_multiplier: f64) -> TextDisplayInfo<'a> {

		Self {
			text: display_text,
			color,
			pixel_area,
			scroll_easer: HashableTextTextureScrollEaser {field: scroll_easer},
			scroll_speed_multiplier: HashableF64 {field: scroll_speed_multiplier}
		}
	}
}

////////// Defining `TextMetadataItem`, and `TextMetadataSet`

#[derive(Clone)]
pub(in crate::texture) struct TextMetadataItem {
	pub(in crate::texture) size: PixelAreaSDL,
	pub(in crate::texture) scroll_speed: f64,
	pub(in crate::texture) scroll_easer: TextTextureScrollEaser,
	pub(in crate::texture) text: String
}

impl TextMetadataItem {
	pub fn maybe_new(texture: &sdl2::render::Texture, creation_info: &pool::TextureCreationInfo) -> Option<Self> {
		// Add/update the metadata key for this handle
		if let pool::TextureCreationInfo::Text((_, text_display_info)) = creation_info {
			let texture_query = texture.query();
			let display_width_to_texture_width_ratio = text_display_info.pixel_area.0 as f64 / texture_query.width as f64;

			Some(TextMetadataItem {
				size: (texture_query.width, texture_query.height),
				scroll_speed: display_width_to_texture_width_ratio * *text_display_info.scroll_speed_multiplier,
				scroll_easer: *text_display_info.scroll_easer,
				text: text_display_info.text.inner().to_string() // TODO: maybe copy it with a reference count instead?
			})
		}
		else {
			None
		}
	}
}

pub(in crate::texture) struct TextMetadataSet {
	metadata: HashMap<pool::TextureHandle, TextMetadataItem>
}

impl TextMetadataSet {
	pub fn new() -> Self {
		Self {metadata: HashMap::new()}
	}

	pub fn get(&self, handle: &pool::TextureHandle) -> Option<&TextMetadataItem> {
		self.metadata.get(handle)
	}

	pub fn contains_handle(&self, handle: &pool::TextureHandle) -> bool {
		self.metadata.contains_key(handle)
	}

	pub fn update(&mut self, handle: &pool::TextureHandle, maybe_item: &Option<TextMetadataItem>) {
		if let Some(item) = maybe_item {
			// Add/update the metadata key for this handle
			self.metadata.insert(handle.clone(), item.clone());
		}
		else {
			/* If it is not text anymore, but text metadata still
			exists for this handle, then remove that metadata.
			TODO: perhaps I could do a font cache clearing here somehow? */
			if self.metadata.contains_key(handle) {
				self.metadata.remove(handle);
			}
		}
	}
}
