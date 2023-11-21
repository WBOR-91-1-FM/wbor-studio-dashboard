use std::collections::HashMap;

use sdl2::{self, ttf, render, rect::Rect, image::LoadTexture};

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
+ font scaling metadata can then go in its own struct.
*/

/* TODO: make a constructor for this, instead of making everything `pub`.
For that, verify that the text to display is not null. */
pub struct TextTextureCreationInfo<'a> {
	pub text_to_display: String,
	pub font_path: &'a str,

	pub style: ttf::FontStyle,
	pub hinting: ttf::Hinting,
	pub color: ColorSDL,

	/* Maps the unix time in secs to a scroll fraction
	(0 to 1), and if the scrolling should wrap. */
	pub scroll_fn: fn(f64) -> (f64, bool),

	pub max_pixel_width: u32,
	pub pixel_height: u32
}

/* TODO: add options for possible color and alpha mods,
and a blend mode (those would go in a struct around this enum) */
pub enum TextureCreationInfo<'a> {
	Path(&'a str),
	Url(&'a str),
	Text(TextTextureCreationInfo<'a>)
}

//////////

/*
Note that the handle is wrapped in a struct, so that it can't be modified.
This can never be copied or cloned, so multiple ownership is not a problem.

Textures can still be lost if they're reassigned (TODO: find some way to avoid that data loss).
TODO: perhaps when doing the remaking thing, pass the handle in as `mut`, even when the handle is not modified (would this help?).
*/

type InnerTextureHandle = u16;

#[derive(Hash, Eq, PartialEq)]
pub struct TextureHandle {
	handle: InnerTextureHandle
}

pub struct SideScrollingTextMetadata {
	size: (u32, u32),
	scroll_fn: fn(f64) -> (f64, bool)
}

/* TODO:

- Later on, if I am using multiple texture pools,
add an id to each texture handle that is meant to match the pool
(to verify that the pool and the handle are only used together)

- Consider using the `unsafe_textures` feature at some point, so that textures can be destroyed
(otherwise, they will eat up all my memory)
*/
pub struct TexturePool<'a> {
	textures: Vec<Texture<'a>>,

	// This maps texture handles of side-scrolling text textures to metadata about that scrolling text
	text_metadata: HashMap<InnerTextureHandle, SideScrollingTextMetadata>,

	texture_creator: &'a TextureCreator,
	ttf_context: &'a ttf::Sdl2TtfContext
}

//////////

type Texture<'a> = render::Texture<'a>;
type TextureCreator = render::TextureCreator<sdl2::video::WindowContext>;
type TextureHandleResult = GenericResult<TextureHandle>;

//////////

/* TODO:
- Can I make one megatexture, and just make handles point to a rect within it?
- Perhaps make the fallback texture a property of the texture pool itself
*/
impl<'a> TexturePool<'a> {
	pub fn new(texture_creator: &'a TextureCreator, ttf_context: &'a ttf::Sdl2TtfContext) -> Self {
		Self {
			textures: Vec::new(),
			text_metadata: HashMap::new(),
			texture_creator,
			ttf_context
		}
	}

	/* This returns the left/righthand screen dest, and a possible other texture
	src and screen dest that may wrap around to the left side of the screen */
	fn split_overflowing_scrolled_rect(texture_src: Rect,
		screen_dest: Rect, texture_size: (u32, u32)) -> (Rect, Option<(Rect, Rect)>) {

		/* Data notes:
		- `texture_src.width == screen_dest.width`
		- `texture_src.height` is almost equal to `screen_dest.height` (let's consider them to be equal)
		- `texture_src.width != texture_width` (`texture_src.width` will be smaller)
		*/

		//////////

		let how_much_wider_the_texture_is_than_its_screen_dest =
			texture_size.0 as i32 - screen_dest.width() as i32;

		std::assert!(how_much_wider_the_texture_is_than_its_screen_dest >= 0);

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

		let texture = self.get_texture_from_handle_immut(handle);
		let possible_text_metadata = self.text_metadata.get(&handle.handle);

		if let None = possible_text_metadata {
			canvas.copy(texture, None, screen_dest)?;
			return Ok(());
		}

		//////////

		// TODO: how can I scroll at the same speed, irrespective of the text size?
		let text_metadata = possible_text_metadata.unwrap();
		let texture_size = text_metadata.size;

		// TODO: compute the time since the unix epoch outside this fn, somehow (or, use the SDL timer)
		let time_since_unix_epoch = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?;
		let secs_since_unix_epoch = time_since_unix_epoch.as_millis() as f64 / 1000.0;

		let (scroll_fract, should_wrap) = (text_metadata.scroll_fn)(secs_since_unix_epoch);
		assert_in_unit_interval(scroll_fract as f32);

		//////////

		let dest_width = screen_dest.width();

		let mut x = texture_size.0;
		if !should_wrap {x -= dest_width;}

		let texture_src = Rect::new(
			((x as f64 * scroll_fract)) as i32,
			0, dest_width, texture_size.1
		);

		if !should_wrap {
			canvas.copy(texture, texture_src, screen_dest)?;
			return Ok(());
		}

		//////////

		let (right_screen_dest, possible_left_rects) = Self::split_overflowing_scrolled_rect(
			texture_src, screen_dest, texture_size
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
			TextureCreationInfo::Text(text_creation_info) => {
				let query = new_texture.query();

				let metadata = SideScrollingTextMetadata {
					size: (query.width, query.height),
					scroll_fn: text_creation_info.scroll_fn
				};

				self.text_metadata.insert(handle.handle, metadata);
			},

			_ => {
				/* If it is not text anymore, but text metadata still
				exists for this handle, then remove that metadata */
				if self.text_metadata.contains_key(&handle.handle) {
					self.text_metadata.remove(&handle.handle);
				}
			}
		}
	}

	pub fn make_texture(&mut self, creation_info: &TextureCreationInfo) -> TextureHandleResult {
		let handle = TextureHandle {handle: (self.textures.len()) as InnerTextureHandle};
		let texture = self.make_raw_texture(creation_info)?;
		self.possibly_update_text_metadata(&texture, &handle, creation_info);
		self.textures.push(texture);
		Ok(handle)
	}

	// TODO: if possible, update the texture in-place instead (if they occupy the amount of space, or less (?))
	pub fn remake_texture(&mut self, handle: &TextureHandle, creation_info: &TextureCreationInfo) -> GenericResult<()> {
		let new_texture = self.make_raw_texture(creation_info)?;

		self.possibly_update_text_metadata(&new_texture, handle, creation_info);
		*self.get_texture_from_handle_mut(handle) = new_texture;

		Ok(())
	}

	// TODO: allow for texture deletion too

	////////// TODO: eliminate the repetition here (inline?)

	fn get_texture_from_handle_mut(&mut self, handle: &TextureHandle) -> &mut Texture<'a> {
		&mut self.textures[handle.handle as usize]
	}

	fn get_texture_from_handle_immut(&self, handle: &TextureHandle) -> &Texture<'a> {
		&self.textures[handle.handle as usize]
	}

	//////////

	fn make_raw_texture(&mut self, creation_info: &TextureCreationInfo) -> GenericResult<Texture<'a>> {
		let texture = match creation_info {
			TextureCreationInfo::Path(path) => {
				self.texture_creator.load_texture(path)
			},

			TextureCreationInfo::Url(url) => {
				/* Normally, the textures are 170x170 (this is described in the URL). If the scale factor isn't a box,
				just the smallest dimension will be picked. But, the size can be modified to anything desired.
				TODO: on the right URL format, resize the image to the given window box size by tweaking the URL
				(but do it from the Spinitron side of things). */

				let response = request::get(url)?; // TODO: make this async
				self.texture_creator.load_texture_bytes(response.as_bytes())
			}

			TextureCreationInfo::Text(info) => {
				const INITIAL_POINT_SIZE: u16 = 100; // TODO: put this in a better place

				////////// Calculating the correct font size

				let initial_font = self.ttf_context.load_font(info.font_path, INITIAL_POINT_SIZE)?;
				let initial_output_size = initial_font.size_of(&info.text_to_display)?;

				// TODO: cache the height ratio in a dict that maps a font name and size to a height ratio
				let height_ratio_from_expected_size = info.pixel_height as f32 / initial_output_size.1 as f32;
				let adjusted_point_size = INITIAL_POINT_SIZE as f32 * height_ratio_from_expected_size;

				// Doing `ceil` here seems to make the output surface's height a tiny bit closer to the desired height
				let nearest_point_size = adjusted_point_size.ceil() as u16;

				////////// Making a font and surface

				let mut font = self.ttf_context.load_font(info.font_path, nearest_point_size)?;
				font.set_style(info.style);
				font.set_hinting(info.hinting.clone());

				let partial_surface = font.render(&info.text_to_display);
				let mut surface = partial_surface.blended(info.color)?;

				////////// Accounting for the case where there is a very small amount of text

				// TODO: don't do this padding thing later

				/* In this case, the text is too small (which will result in it being
				stretched out otherwise). For that, I am adding some blank padding. */
				if info.max_pixel_width > surface.width() {
					let mut with_padding_on_right = sdl2::surface::Surface::new(
						info.max_pixel_width, surface.height(),
						surface.pixel_format_enum()
					)?;

					surface.set_blend_mode(render::BlendMode::None)?;
					surface.blit(None, &mut with_padding_on_right, None)?;
					surface = with_padding_on_right;
				}

				/* The surface height here will be very close to the expected height,
				but not exactly the same. It may be off by a pixel or two. */

				////////// Making and returning a finished texture

				Ok(self.texture_creator.create_texture_from_surface(surface)?)
			}
		};

		Ok(texture?)
	}
}
