use std::{
	borrow::Cow,
	collections::HashMap
};

use sdl2::{
	ttf,
	rect::Rect,
	surface::Surface,
	image::LoadTexture,
	render::{self, Texture}
};

use crate::{
	request,
	window_tree::{CanvasSDL, ColorSDL},

	utility_types::{
		file_utils,
		generic_result::*,
		vec2f::assert_in_unit_interval
	}
};

//////////

/* TODO: put a lot of the text-related code in its own file
(this file can then import that one).
The needed structs + data can go there, and the text
+ font scaling metadata can then go in its own struct. */

// TODO: make a constructor for this, instead of making everything `pub`.
#[derive(Clone)]
pub struct FontInfo {
	/* TODO:
	- Support non-static paths for these two
	- Allow for a variable number of fallback fonts too
	- Only load fallbacks when necessary
	- Check if the entire Unicode supplementary multilingual plane is supported
	*/
	pub path: &'static str,
	pub unusual_chars_fallback_path: &'static str,

	pub font_has_char: fn(&ttf::Font, char) -> bool,

	pub style: ttf::FontStyle,
	pub hinting: ttf::Hinting,
	pub maybe_outline_width: Option<u16>
}

#[derive(Clone)]
pub struct DisplayText<'a> {
	text: Cow<'a, str>
}

impl<'a> DisplayText<'a> {
	pub fn new(text: &str) -> Self {
		// Indicates that emojis should be made colored; not rendered correctly on the Pi
		const UNICODE_VARIATION_SELECTOR_16: char = '\u{FE0F}';

		const WHITESPACE_REPLACEMENT_PAIRS: [(char, &str); 3] = [
			('\t', "    "),
			('\n', " "),
			(UNICODE_VARIATION_SELECTOR_16, "")
		];

		/* TODO:
		- Should I add the rest of the blank characters (see https://invisible-characters.com/ for all), for better cleanup?
		- The second reason for this is to stop 'nonavailable' character variants to appear - although this would be hard to verify
		*/
		const ALL_WHITESPACE_CHARS: [char; 4] = [
			' ', '\t', '\n', UNICODE_VARIATION_SELECTOR_16
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
}

//////////

/* Input: seed, and if the text fits fully in the box.
Output: scroll amount (in [0, 1]), and if the text should wrap or not. */
pub type TextTextureScrollFn = fn(f64, bool) -> (f64, bool);

// TODO: make a constructor for this, instead of making everything `pub`.
#[derive(Clone)]
pub struct TextDisplayInfo<'a> {
	pub text: DisplayText<'a>,
	pub color: ColorSDL, // TODO: change the name of this to `text_color`, perhaps
	pub pixel_area: (u32, u32),

	/* Maps the unix time in secs to a scroll fraction
	(0 to 1), and if the scrolling should wrap. */
	pub scroll_fn: TextTextureScrollFn
}

// TODO: use `Cow` around the whole struct instead, if possible
#[derive(Clone)]
pub enum TextureCreationInfo<'a> {
	RawBytes(Cow<'a, [u8]>),
	Path(Cow<'a, str>),
	Url(Cow<'a, str>),
	Text((Cow<'a, FontInfo>, TextDisplayInfo<'a>))
}

impl TextureCreationInfo<'_> {
	fn raw_bytes(contents: Vec<u8>) -> GenericResult<Self> {
		Ok(TextureCreationInfo::RawBytes(Cow::Owned(contents)))
	}

	pub fn from_path(path: &str) -> TextureCreationInfo<'_> {
		TextureCreationInfo::Path(Cow::Borrowed(path))
	}

	// This function preprocesses the texture creation info into a form that can be loaded quicker in non-async contexts
	pub async fn from_path_async(path: &str) -> GenericResult<Self> {
		Self::raw_bytes(file_utils::read_file_contents(path).await?)
	}

	// The same applies for this one, but it does it for for multiple paths concurrently
	pub async fn from_paths_async<'b>(paths: impl IntoIterator<Item = &'b str>) -> GenericResult<Vec<TextureCreationInfo<'b>>> {
		let contents_future_iterator = paths.into_iter().map(TextureCreationInfo::from_path_async);
		futures::future::try_join_all(contents_future_iterator).await
	}
}

//////////

/*
- Note that the handle is wrapped in a struct, so that it can't be modified.
- Multiple ownership is possible, since we can clone the handles.
- Textures can still be lost if they're reassigned (TODO: find some way to avoid that data loss. Also, can I detect this loss with `Rc` somehow?)
- TODO: perhaps when doing the remaking thing, pass the handle in as `mut`, even when the handle is not modified (would this help?).
*/

type InnerTextureHandle = u16;
type TextureCreator = render::TextureCreator<sdl2::video::WindowContext>;

type FontPointSize = u16;

// Font path for default, font path for fallback, point size for default, point size for fallback
type FontCacheKey = (&'static str, &'static str, FontPointSize, FontPointSize);
type FontPair<'a> = (ttf::Font<'a, 'a>, ttf::Font<'a, 'a>);

#[derive(Hash, Eq, PartialEq, Clone)]
pub struct TextureHandle {
	handle: InnerTextureHandle
}

struct SideScrollingTextMetadata {
	size: (u32, u32),
	scroll_fn: TextTextureScrollFn,
	text: String
}

/* TODO:
- Later on, if I am using multiple texture pools,
add an id to each texture handle that is meant to match the pool
(to verify that the pool and the handle are only used together).
Otherwise, try to find some way to verify that it's a singleton.

- Will textures be destroyed when dropped currently, and if so, would using
the `unsafe_textures` feature help this?
*/

pub struct TexturePool<'a> {
	max_texture_size: (u32, u32),
	textures: Vec<Texture<'a>>,
	texture_creator: &'a TextureCreator,

	//////////

	ttf_context: &'a ttf::Sdl2TtfContext,

	// This maps font paths and point sizes to fonts (TODO: should I limit the cache size?)
	font_cache: HashMap<FontCacheKey, FontPair<'a>>,

	// This maps texture handles of side-scrolling text textures to metadata about that scrolling text
	text_metadata: HashMap<TextureHandle, SideScrollingTextMetadata>
}

//////////

/* TODO:
- Can I make one megatexture, and just make handles point to a rect within it?
- Perhaps make the fallback texture a property of the texture pool itself
- Would it make sense to make a trait called `TextureRenderingMethod` for normal textures and fonts? That might make this code cleaner
*/
impl<'a> TexturePool<'a> {
	const INITIAL_POINT_SIZE: FontPointSize = 100;
	const BLANK_TEXT_DEFAULT: &'static str = "<BLANK TEXT>";

	pub fn new(texture_creator: &'a TextureCreator,
		ttf_context: &'a ttf::Sdl2TtfContext,
		max_texture_size: (u32, u32)) -> Self {

		Self {
			max_texture_size,
			textures: Vec::new(),
			texture_creator,

			ttf_context,
			text_metadata: HashMap::new(),
			font_cache: HashMap::new()
		}
	}

	pub fn is_text_texture(&self, handle: &TextureHandle) -> bool {
		self.text_metadata.contains_key(handle)
	}

	// TODO: cache this
	pub fn get_aspect_ratio_for(&self, handle: &TextureHandle) -> f32 {
		let query = self.get_texture_from_handle(handle).query();
		query.width as f32 / query.height as f32
	}

	/*
	pub fn size(&self) -> usize {
		self.textures.len()
	}
	*/

	/* This returns the left/righthand screen dest, and a possible other texture
	src and screen dest that may wrap around to the left side of the screen */
	fn split_overflowing_scrolled_rect(
		texture_src: Rect, screen_dest: Rect,
		texture_size: (u32, u32),
		text: &str) -> (Rect, Option<(Rect, Rect)>) {

		/* Input data notes:
		- `texture_src.width == screen_dest.width`
		- `texture_src.height` == `screen_dest.height`
		- `texture_src.width != texture_width` (`texture_src.width` will be smaller or equal)
		*/

		//////////

		/* TODO: why does this bug still happen on MacOS with the multi-monitor setup?
		Perhaps from monitor shutoff -> app moves to being displayed on the laptop screen -> resolution change?
		Test this overnight with no automatic standby, and with automatic standby, to track the time at which this happened. */
		let how_much_wider_the_texture_is_than_its_screen_dest =
			texture_size.0 as i32 - screen_dest.width() as i32;

		if how_much_wider_the_texture_is_than_its_screen_dest < 0 {
			panic!("The texture was not wider than its screen dest, which will yield incorrect results.\n\
				Difference = {how_much_wider_the_texture_is_than_its_screen_dest}. Texture src = {texture_src:?}, \
				screen dest = {screen_dest:?}. The text was '{text}'.");
		}

		/* If the texture can be cropped so that it ends up fully
		on the left side, without spilling onto the right */
		if texture_src.x() <= how_much_wider_the_texture_is_than_its_screen_dest {
			return (screen_dest, None);
		}

		//////////

		// The texture will spill over by this amount otherwise (onto the left side)
		let texture_right_side_spill_amount =
			(texture_src.x() - how_much_wider_the_texture_is_than_its_screen_dest) as u32;

		let (mut lefthand_screen_dest, mut righthand_dest_rect) = (screen_dest, screen_dest);

		righthand_dest_rect.set_width(screen_dest.width() - texture_right_side_spill_amount);
		lefthand_screen_dest.set_width(texture_right_side_spill_amount);
		lefthand_screen_dest.set_x(righthand_dest_rect.right());

		//////////

		let lefthand_texture_clip_rect = Rect::new(
			0, 0, texture_right_side_spill_amount, texture_size.1
		);

		(righthand_dest_rect, Some((lefthand_texture_clip_rect, lefthand_screen_dest)))
	}

	/* TODO:
	- Add an option for not scrolling text (a fixed string that never changes)
	- Make the scroll effect something common?
	- Would it be possible to manipulate the canvas scale to be able to only pass normalized coordinates to the renderer?
	- Use `copy_ex` eventually, and the special canvas functions for things like rounded rectangles
	*/
	pub fn draw_texture_to_canvas(&self, handle: &TextureHandle,
		canvas: &mut CanvasSDL, screen_dest: Rect) -> MaybeError {

		let texture = self.get_texture_from_handle(handle);
		let possible_text_metadata = self.text_metadata.get(handle);

		if possible_text_metadata.is_none() {
			return canvas.copy(texture, None, screen_dest).to_generic();
		}

		//////////

		let text_metadata = possible_text_metadata.context("Expected text metadata")?;
		let texture_size = text_metadata.size;

		// TODO: compute the time since the unix epoch outside this fn, somehow (or, use the SDL timer)

		let dest_width = screen_dest.width();
		let time_since_unix_epoch = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?;
		let time_seed = (time_since_unix_epoch.as_millis() as f64 / 1000.0) * (dest_width as f64 / texture_size.0 as f64);

		let mut x = texture_size.0;

		let (scroll_fract, should_wrap) = (text_metadata.scroll_fn)(
			time_seed, x <= dest_width
		);

		assert_in_unit_interval(scroll_fract as f32);

		//////////

		if !should_wrap {x -= dest_width;}

		//////////

		let texture_src = Rect::new(
			(x as f64 * scroll_fract) as i32,
			0, dest_width, texture_size.1
		);

		if !should_wrap {
			return canvas.copy(texture, texture_src, screen_dest).to_generic();
		}

		//////////

		let (right_screen_dest, possible_left_rects) = Self::split_overflowing_scrolled_rect(
			texture_src, screen_dest, texture_size, &text_metadata.text
		);

		canvas.copy(texture, texture_src, right_screen_dest).to_generic()?;

		if let Some((left_texture_src, left_screen_dest)) = possible_left_rects {
			canvas.copy(texture, left_texture_src, left_screen_dest).to_generic()?;
		}

		Ok(())
	}

	fn possibly_update_text_metadata(&mut self, new_texture: &Texture,
		handle: &TextureHandle, creation_info: &TextureCreationInfo) {

		match creation_info {
			// Add/update the metadata key for this handle
			TextureCreationInfo::Text((_, text_display_info)) => {
				let query = new_texture.query();

				let metadata = SideScrollingTextMetadata {
					size: (query.width, query.height),
					scroll_fn: text_display_info.scroll_fn,
					text: text_display_info.text.text.to_string() // TODO: maybe copy it with a reference count instead?
				};

				self.text_metadata.insert(handle.clone(), metadata);
			},

			_ => {
				/* If it is not text anymore, but text metadata still
				exists for this handle, then remove that metadata.
				TODO: perhaps I could do a font cache clearing here somehow? */
				if self.text_metadata.contains_key(handle) {
					self.text_metadata.remove(handle);
				}
			}
		}
	}

	//////////

	pub fn make_texture(&mut self, creation_info: &TextureCreationInfo) -> GenericResult<TextureHandle> {
		let handle = TextureHandle {handle: self.textures.len() as InnerTextureHandle};
		let texture = self.make_raw_texture(creation_info)?;

		self.possibly_update_text_metadata(&texture, &handle, creation_info);
		self.textures.push(texture);

		Ok(handle)
	}

	// TODO: if possible, update the texture in-place instead (if they occupy the amount of space, or less)
	pub fn remake_texture(&mut self, creation_info: &TextureCreationInfo, handle: &TextureHandle) -> MaybeError {
		let new_texture = self.make_raw_texture(creation_info)?;

		self.possibly_update_text_metadata(&new_texture, handle, creation_info);
		*self.get_texture_from_handle_mut(handle) = new_texture;

		Ok(())
	}

	// TODO: allow for texture deletion too

	////////// TODO: use these

	/*
	pub fn set_color_mod_for(&mut self, handle: &TextureHandle, r: u8, g: u8, b: u8) {
		self.get_texture_from_handle_mut(handle).set_color_mod(r, g, b);
	}

	pub fn set_alpha_mod_for(&mut self, handle: &TextureHandle, a: u8) {
		self.get_texture_from_handle_mut(handle).set_alpha_mod(a);
	}
	*/

	pub fn set_blend_mode_for(&mut self, handle: &TextureHandle, blend_mode: render::BlendMode) {
		self.get_texture_from_handle_mut(handle).set_blend_mode(blend_mode);
	}

	////////// TODO: eliminate the repetition here (perhaps inline, or make to a macro - or is there some other way?)

	fn get_texture_from_handle(&self, handle: &TextureHandle) -> &Texture<'a> {
		&self.textures[handle.handle as usize]
	}

	fn get_texture_from_handle_mut(&mut self, handle: &TextureHandle) -> &mut Texture<'a> {
		&mut self.textures[handle.handle as usize]
	}

	//////////

	fn get_font_pair(&mut self, key: FontCacheKey, maybe_options: Option<&FontInfo>) -> &FontPair {
		let fonts = self.font_cache.entry(key).or_insert_with(
			|| {
				// TODO: don't unwrap
				let make_font = |path, point_size| self.ttf_context.load_font(path, point_size).unwrap();
				let (default_path, fallback_path, default_point_size, fallback_point_size) = key;
				(make_font(default_path, default_point_size), make_font(fallback_path, fallback_point_size))
			}
		);

		if let Some(options) = maybe_options {
			let set_options = |font: &mut ttf::Font| {
				font.set_style(options.style);
				font.set_hinting(options.hinting.clone());

				if let Some(outline_width) = options.maybe_outline_width {
					font.set_outline_width(outline_width);
				}
			};

			set_options(&mut fonts.0);
			set_options(&mut fonts.1);
		}

		fonts
	}

	fn get_point_and_surface_size_for_initial_font(initial_font: &ttf::Font,
		text_display_info: &TextDisplayInfo) -> GenericResult<(FontPointSize, (u32, u32))> {

		let initial_output_size = initial_font.size_of(&text_display_info.text.text)?;

		let height_ratio_from_expected_size = text_display_info.pixel_area.1 as f64 / initial_output_size.1 as f64;
		let adjusted_point_size = Self::INITIAL_POINT_SIZE as f64 * height_ratio_from_expected_size;

		// TODO: would it work better if I used `round` or `ceil` for the adjsuted point size instead?
		Ok((adjusted_point_size as FontPointSize, initial_output_size))
	}

	//////////

	/* Assuming that the passed-in text will not result in a zero-width
	surface (that is handled in `make_text_surface`). */
	fn inner_make_text_surface(text_display_info: &TextDisplayInfo,
		font_pair: &FontPair, font_has_char: fn(&ttf::Font, char) -> bool,
		max_texture_width: u32) -> GenericResult<Surface<'a>> {

		let chars: Vec<char> = text_display_info.text.text.chars().collect();
		let num_chars = chars.len();

		let (default_font, fallback_font) = font_pair;

		let (mut i, mut total_surface_width, mut max_surface_height, mut subsurfaces) = (0, 0, 0, Vec::new());

		while i != num_chars {
			let (use_plain_font, start) = (font_has_char(default_font, chars[i]), i);

			while i != num_chars && font_has_char(default_font, chars[i]) == use_plain_font {
				i += 1;
			}

			let chosen_font = if use_plain_font {default_font} else {fallback_font};

			let compute_span_data = |span: &[char]| -> GenericResult<(String, u32, u32)> {
				let span_as_string = span.iter().collect::<String>();
				let subsurface_width = chosen_font.size_of(&span_as_string)?.0;
				let next_total_width = total_surface_width + subsurface_width;

				Ok((span_as_string, subsurface_width, next_total_width))
			};

			//////////

			let mut span = &chars[start..i];
			let (mut span_as_string, mut subsurface_width, mut next_total_width) = compute_span_data(span)?;

			// Not checking for an empty string earlier, since empty Unicode characters can exist
			if subsurface_width == 0 {
				log::debug!("Text subsurface with zero width; ignoring it");
				continue;
			}

			//////////

			let text_goes_over_max_width = next_total_width > max_texture_width;

			if text_goes_over_max_width {
				log::debug!("A subsurface exceeded the pixel width maximum (the next total was {next_total_width}); will try to trim it");

				/* If the font is monospace (and not italicized) and it exceeds the
				max texture width, cut off enough characters to make it fit in one texture.
				I am not running this branch for italicized fonts since italicized fonts are
				not really monospaced per character. */
				if chosen_font.face_is_fixed_width() && !chosen_font.get_style().intersects(ttf::FontStyle::ITALIC) {
					log::debug!("Doing optimized monospace text span cutting");

					let orig_span_len = span.len();
					let first_char_pixel_width = chosen_font.size_of_char(span[0])?.0;

					// Checking that the monospace property holds
					assert!(first_char_pixel_width * orig_span_len as u32 == subsurface_width);

					let pixel_overstep = next_total_width - max_texture_width;
					let approx_char_overstep = pixel_overstep as f64 / subsurface_width as f64 * orig_span_len as f64;
					let char_overstep = approx_char_overstep.ceil() as usize;

					// Checking that the cut text amount is not too large for this span
					assert!(char_overstep <= orig_span_len);

					span = &span[0..orig_span_len - char_overstep];
					(span_as_string, subsurface_width, next_total_width) = compute_span_data(span)?;

					// Double-checking that the monospace property holds
					assert!(subsurface_width == first_char_pixel_width * span.len() as u32);
				}
				else {
					log::debug!("Font was not monospaced; doing manual text span cutting");

					while next_total_width > max_texture_width {
						log::debug!("Doing an iteration of manual inefficient text span cutting");
						span = &span[0..span.len() - 1];
						(span_as_string, subsurface_width, next_total_width) = compute_span_data(span)?;
					}
				}

				/////////

				log::debug!("Final cut width = {next_total_width} (checking if it is under or equal to the limit of {max_texture_width})");
				assert!(next_total_width <= max_texture_width);

				if subsurface_width == 0 {
					log::debug!("Zero-width subsurface width after text cutting; ignoring it");
					break;
				}
			}

			//////////

			let subsurface = chosen_font.render(&span_as_string).blended(text_display_info.color)?;
			assert!(subsurface_width == subsurface.width());

			total_surface_width += subsurface_width;
			max_surface_height = max_surface_height.max(subsurface.height());
			subsurfaces.push(subsurface);

			if text_goes_over_max_width {
				log::debug!("Stopping the text-texture-generation early after doing the text cutting");
				break;
			}
		}

		//////////

		/* TODO:
		- Add ellipses at the end
		- Support multiline text (cut it off at some point though)
		- Why is the text height so incorrect right now for fullscreen mode on Fedora?
		- Can I avoid doing right padding or bottom cutting if I just do a plain blit somehow from the rendering code?
		*/

		let pixel_height = text_display_info.pixel_area.1;

		/*
		if pixel_height != max_surface_height {
			log::debug!("Doing slight text texture height correction (adjusting {max_surface_height} to {pixel_height})");
		}
		*/

		let mut joined_surface = Surface::new(
			total_surface_width.max(text_display_info.pixel_area.0),
			pixel_height, subsurfaces[0].pixel_format_enum()
		).to_generic()?;

		let mut dest_rect = Rect::new(0, 0, 1, 1);

		for mut subsurface in subsurfaces {
			subsurface.set_blend_mode(render::BlendMode::None).to_generic()?;

			(dest_rect.w, dest_rect.h) = (subsurface.width() as i32, subsurface.height() as i32);
			subsurface.blit(None, &mut joined_surface, dest_rect).to_generic()?;
			dest_rect.x += dest_rect.w;
		}

		Ok(joined_surface)
	}

	fn make_text_surface(&mut self, font_info: &FontInfo,
		text_display_info: &TextDisplayInfo) -> GenericResult<Surface<'a>> {

		////////// First, getting a point size

		let max_texture_width = self.max_texture_size.0;

		let (initial_default_font, initial_fallback_font) = self.get_font_pair(
			(font_info.path, font_info.unusual_chars_fallback_path, Self::INITIAL_POINT_SIZE, Self::INITIAL_POINT_SIZE), None
		);

		let ((default_point_size, initial_default_output_size),
			(fallback_point_size, initial_fallback_output_size)) = (

			Self::get_point_and_surface_size_for_initial_font(initial_default_font, text_display_info)?,
			Self::get_point_and_surface_size_for_initial_font(initial_fallback_font, text_display_info)?
		);

		////////// Second, making a font pair

		let font_pair = self.get_font_pair(
			(font_info.path, font_info.unusual_chars_fallback_path, default_point_size, fallback_point_size), Some(font_info)
		);

		////////// Early exit point: if the font turned out to have zero width, then make a blank text surface

		let (max_width, needed_height) = text_display_info.pixel_area;

		// Not checking for an empty string earlier, since empty Unicode characters can exist
		if initial_default_output_size.0 == 0 || initial_fallback_output_size.0 == 0 {
			log::debug!("Making a blank-text-default text texture");

			let mut blank_surface = font_pair.0.render(Self::BLANK_TEXT_DEFAULT).blended(text_display_info.color)?;

			Ok(if blank_surface.width() < max_width || blank_surface.height() != needed_height {
				let mut corrected = Surface::new(max_width, needed_height, blank_surface.pixel_format_enum()).to_generic()?;
				blank_surface.set_blend_mode(render::BlendMode::None).to_generic()?;
				blank_surface.blit(None, &mut corrected, None).to_generic()?;
				corrected
			}
			else {
				blank_surface
			})
		}
		else {
			Self::inner_make_text_surface(text_display_info, font_pair, font_info.font_has_char, max_texture_width)
		}
	}

	//////////

	fn make_raw_texture(&mut self, creation_info: &TextureCreationInfo) -> GenericResult<Texture<'a>> {
		match creation_info {
			// Use this whenever possible (whenever you can preload data into byte form)!
			TextureCreationInfo::RawBytes(bytes) =>
				self.texture_creator.load_texture_bytes(bytes),

			TextureCreationInfo::Path(path) =>
				self.texture_creator.load_texture(path as &str),

			TextureCreationInfo::Url(url) => {
				use futures::executor::block_on;

				let response = block_on(request::get(url))?;
				let bytes = block_on(response.bytes())?;

				self.texture_creator.load_texture_bytes(&bytes)
			}

			TextureCreationInfo::Text((font_info, text_display_info)) => {
				let surface = self.make_text_surface(font_info, text_display_info)?;

				assert!(surface.width() >= text_display_info.pixel_area.0);
				assert!(surface.height() == text_display_info.pixel_area.1);

				Ok(self.texture_creator.create_texture_from_surface(surface)?)
			}
		}.to_generic()
	}
}
