use std::{borrow::Cow, collections::HashMap};
use sdl2::{self, ttf, image::LoadTexture, rect::Rect, render::{self, Texture}};

use crate::{
	request,

	window_tree::{CanvasSDL, ColorSDL},

	utility_types::{
		generic_result::GenericResult,
		vec2f::assert_in_unit_interval
	}
};

//////////

/* TODO: put a lot of the text-related code in its own file
(this file can then import that one).
The needed structs + data can go there, and the text
+ font scaling metadata can then go in its own struct. */

/* Input: seed, and if the text fits fully in the box.
Output: scroll amount (in [0, 1]), and if the text should wrap or not. */
pub type TextTextureScrollFn = fn(f64, bool) -> (f64, bool);

// TODO: make a constructor for this, instead of making everything `pub`.
#[derive(Clone)]
pub struct FontInfo {
	pub path: &'static str, // TODO: support non-static paths for this
	pub style: ttf::FontStyle,
	pub hinting: ttf::Hinting,
	pub maybe_outline_width: Option<u16>
}

// TODO: make a constructor for this, instead of making everything `pub`.
#[derive(Clone)]
pub struct TextDisplayInfo<'a> {
	pub text: Cow<'a, str>,
	pub color: ColorSDL,

	/* Maps the unix time in secs to a scroll fraction
	(0 to 1), and if the scrolling should wrap. */
	pub scroll_fn: TextTextureScrollFn,

	pub max_pixel_width: u32,
	pub pixel_height: u32
}

#[derive(Clone)]
pub enum TextureCreationInfo<'a> {
	Path(Cow<'a, str>),
	Url(Cow<'a, str>),
	Text((&'a FontInfo, TextDisplayInfo<'a>))
}

//////////

/*
- Note that the handle is wrapped in a struct, so that it can't be modified.
- Multiple ownership is possible, since we can clone the handles.
- Textures can still be lost if they're reassigned (TODO: find some way to avoid that data loss).
- TODO: perhaps when doing the remaking thing, pass the handle in as `mut`, even when the handle is not modified (would this help?). */

type InnerTextureHandle = u16;
type TextureCreator = render::TextureCreator<sdl2::video::WindowContext>;

#[derive(Hash, Eq, PartialEq, Clone)] // TODO: remove `Clone`
pub struct TextureHandle {
	handle: InnerTextureHandle
}

pub struct SideScrollingTextMetadata {
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
	font_cache: HashMap<(&'static str, u16), ttf::Font<'a, 'a>>,

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
	const INITIAL_POINT_SIZE: u16 = 100;
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

		let how_much_wider_the_texture_is_than_its_screen_dest =
			texture_size.0 as i32 - screen_dest.width() as i32;

		if how_much_wider_the_texture_is_than_its_screen_dest < 0 {
			panic!("The texture was not wider than its screen dest, which will yield incorrect results.\n\
				Difference = {how_much_wider_the_texture_is_than_its_screen_dest}. Texture src = {:?}, \
				screen dest = {:?}. The text was '{text}'.", texture_src, screen_dest);
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
	- Would it be possible to manipulate the canvas scale to be able to only pass normalized coordinates to the renderer?
	- Make the scroll effect something common?
	*/
	pub fn draw_texture_to_canvas(&self, handle: &TextureHandle,
		canvas: &mut CanvasSDL, screen_dest: Rect) -> GenericResult<()> {

		let texture = self.get_texture_from_handle(handle);
		let possible_text_metadata = self.text_metadata.get(handle);

		if possible_text_metadata.is_none() {
			canvas.copy(texture, None, screen_dest)?;
			return Ok(());
		}

		//////////

		let text_metadata = possible_text_metadata.ok_or("Expected text metadata")?;
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
			canvas.copy(texture, texture_src, screen_dest)?;
			return Ok(());
		}

		//////////

		let (right_screen_dest, possible_left_rects) = Self::split_overflowing_scrolled_rect(
			texture_src, screen_dest, texture_size, &text_metadata.text
		);

		canvas.copy(texture, texture_src, right_screen_dest)?;

		if let Some((left_texture_src, left_screen_dest)) = possible_left_rects {
			canvas.copy(texture, left_texture_src, left_screen_dest)?;
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
					text: text_display_info.text.to_string() // TODO: maybe copy it with a reference count instead?
				};

				self.text_metadata.insert(handle.clone(), metadata);
			},

			_ => {
				/* If it is not text anymore, but text metadata still
				exists for this handle, then remove that metadata */
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
	pub fn remake_texture(&mut self, creation_info: &TextureCreationInfo, handle: &TextureHandle) -> GenericResult<()> {
		let new_texture = self.make_raw_texture(creation_info)?;

		self.possibly_update_text_metadata(&new_texture, handle, creation_info);
		*self.get_texture_from_handle_mut(handle) = new_texture;

		Ok(())
	}

	// TODO: allow for texture deletion too

	////////// TODO: use these

	pub fn set_color_mod_for(&mut self, handle: &TextureHandle, r: u8, g: u8, b: u8) {
		let texture = self.get_texture_from_handle_mut(handle);
		texture.set_color_mod(r, g, b);
	}

	pub fn set_alpha_mod_for(&mut self, handle: &TextureHandle, a: u8) {
		let texture = self.get_texture_from_handle_mut(handle);
		texture.set_alpha_mod(a);
	}

	pub fn set_blend_mode_for(&mut self, handle: &TextureHandle, blend_mode: render::BlendMode) {
		let texture = self.get_texture_from_handle_mut(handle);
		texture.set_blend_mode(blend_mode);
	}

	////////// TODO: eliminate the repetition here (perhaps inline, or make to a macro - or is there some other way?)

	fn get_texture_from_handle_mut(&mut self, handle: &TextureHandle) -> &mut Texture<'a> {
		&mut self.textures[handle.handle as usize]
	}

	fn get_texture_from_handle(&self, handle: &TextureHandle) -> &Texture {
		&self.textures[handle.handle as usize]
	}

	//////////

	fn get_cached_font(&mut self, path: &'static str, point_size: u16,
		maybe_options: Option<&FontInfo>) -> &ttf::Font {

		// TODO: don't unwrap
		let font = self.font_cache.entry((path, point_size)).or_insert_with(
			|| self.ttf_context.load_font(path, point_size).unwrap()
		);

		if let Some(options) = maybe_options {
			font.set_style(options.style);
			font.set_hinting(options.hinting.clone());

			if let Some(outline_width) = options.maybe_outline_width {
				font.set_outline_width(outline_width);
			}
		}

		font
	}

	fn get_font_with_processed_text<'b>(
		&mut self, font_info: &FontInfo,
		text_display_info: &'b TextDisplayInfo)

		-> GenericResult<(&'b ttf::Font, &'b str)> {

		////////// First, getting a point size

		// Blank text can't be rendered by SDL, so handling that here
		let text = if text_display_info.text == "" {Self::BLANK_TEXT_DEFAULT} else {&text_display_info.text};

		let initial_font = self.get_cached_font(font_info.path, Self::INITIAL_POINT_SIZE, None);
		let initial_output_size = initial_font.size_of(text)?; // TODO: can/should I use a unicode variant for emoji rendering then? And maybe only render the first character?

		// TODO: can I cache this height ratio for any text, or would it vary based on the text?
		let height_ratio_from_expected_size = text_display_info.pixel_height as f64 / initial_output_size.1 as f64;
		let adjusted_point_size = Self::INITIAL_POINT_SIZE as f64 * height_ratio_from_expected_size;

		// Flooring this makes the assertions at the end of `make_raw_texture` always succeed on MacOS
		let nearest_point_size = adjusted_point_size as u16;

		////////// Second, making a font

		let max_texture_width = self.max_texture_size.0;

		let font = self.get_cached_font(
			font_info.path, nearest_point_size, Some(&font_info)
		);

		////////// Third, cutting the text if it becomes too long

		let initial_texture_width = font.size_of(text)?.0;

		let cut_text = if initial_texture_width > max_texture_width {
			// println!("Cutting texture text because it is too long.");

			let ratio_over_max_width = max_texture_width as f64 / initial_texture_width as f64;
			let amount_chars_to_keep = (text.len() as f64 * ratio_over_max_width) as usize;
			let text_slice = &text[..amount_chars_to_keep];

			let cut_texture_width = font.size_of(text_slice)?.0;
			assert!(cut_texture_width <= max_texture_width);

			text_slice
		}
		else {
			text
		};

		////////// Fourth, returning the font and the processed text

		Ok((font, cut_text))
	}

	fn make_raw_texture(&mut self, creation_info: &TextureCreationInfo) -> GenericResult<Texture<'a>> {
		let texture = match creation_info {
			TextureCreationInfo::Path(path) => {
				self.texture_creator.load_texture(path as &str)
			},

			// TODO: could I pass an optional texture-rescaling param here for the Spinitron spin textures (instead of in the model logic?)
			TextureCreationInfo::Url(url) => {
				let response = request::get(url)?;
				self.texture_creator.load_texture_bytes(response.as_bytes())
			}

			TextureCreationInfo::Text((font_info, text_display_info)) => {
				////////// Making a surface

				let mut surface = {
					let (font, processed_text) = self.get_font_with_processed_text(font_info, text_display_info)?;
					font.render(processed_text).blended(text_display_info.color)?
				};

				////////// Accounting for the case where there is a very small amount of text, or the surface height doesn't match


				// TODO: can I avoid doing right padding or bottom cutting if I just do a plain blit somehow from the rendering code?
				let surface_is_too_short = surface.width() < text_display_info.max_pixel_width;
				let text_height_doesnt_match = surface.height() != text_display_info.pixel_height;

				// TODO: why is the text height so incorrect right now for fullscreen mode on Fedora?
				if surface_is_too_short || text_height_doesnt_match {
					let dimensions;

					// With padding on right, and slightly changed height
					if surface_is_too_short && text_height_doesnt_match {
						// println!("Add padding to right, and height is off");
						dimensions = (text_display_info.max_pixel_width, text_display_info.pixel_height);
					}
					// With padding on right
					else if surface_is_too_short {
						// println!("Add padding to right");
						dimensions = (text_display_info.max_pixel_width, surface.height());
					}
					// With slightly changed height
					else if text_height_doesnt_match {
						// println!("Height is off");
						dimensions = (surface.width(), text_display_info.pixel_height);
					}
					else {
						panic!("Impossible text texture rescaling situation!");
					}

					let mut resized_dest = sdl2::surface::Surface::new(
						dimensions.0, dimensions.1, surface.pixel_format_enum()
					)?;

					surface.set_blend_mode(render::BlendMode::None)?;
					surface.blit(None, &mut resized_dest, None)?;
					surface = resized_dest;
				}

				assert!(surface.width() >= text_display_info.max_pixel_width);
				assert!(surface.height() == text_display_info.pixel_height);

				////////// Making and returning a finished texture

				Ok(self.texture_creator.create_texture_from_surface(surface)?)
			}
		};

		Ok(texture?)
	}
}
