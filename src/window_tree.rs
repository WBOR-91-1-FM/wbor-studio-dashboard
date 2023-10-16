use sdl2;

use crate::vec2f::{assert_in_unit_interval, Vec2f};
use crate::texture;
use crate::dynamic_optional;
use crate::generic_result::GenericResult;

////////// These are some general utilities

pub type ColorSDL = sdl2::pixels::Color;
pub type CanvasSDL = sdl2::render::Canvas<sdl2::video::Window>;

type InnerWindowUpdater = fn(&mut Window, &mut texture::TexturePool) -> GenericResult<()>;
type WindowUpdater = Option<InnerWindowUpdater>;

//////////

pub enum WindowContents {
	Color(ColorSDL),
	Texture(texture::TextureHandle)
}

impl WindowContents {
	pub fn make_color(r: u8, g: u8, b: u8) -> Self {
		Self::Color(ColorSDL::RGB(r, g, b))
	}

	// `a` ranges from 0 to 1
	pub fn make_transparent_color(r: u8, g: u8, b: u8, a: f32) -> Self {
		assert_in_unit_interval(a);
		Self::Color(ColorSDL::RGBA(r, g, b, (a * 255.0) as u8))
	}
}

pub struct Window {
	/* TODO: set an optional poll rate for some functions (how to express it?)
	Brainstorming:
	- First, no poll rate (update every second)
	- Second, express it as a fraction of the refresh rate
	- Third, express it as its own rate (independent from that rate)

	- The third idea might be the best
	- For that, make some function that tells you if the updater is allowed to refresh

	- Hm, the third idea might not work, if the rate is higher than the refresh rate
	- A simple solution would be to say - skip every N frames, and then call it
	- So some ratio of the refresh rate
	- Maybe I'll start with that
	*/

	updater: WindowUpdater,

	pub state: dynamic_optional::DynamicOptional,
	pub contents: WindowContents,

	top_left: Vec2f,
	bottom_right: Vec2f,

	/* TODO: makybe do splitting here instead. Ideas for that:
	KD-tree:
	- Splitting axis would alternate per each box level
	- Ideally, I would make it not alternate (is that possible?)
	- And having multiple boxes per box (in an efficient manner) would not be possible for that

	Other idea:
	```
	struct SplitBox {
		is_on_vertical_axis: bool,
		split_spacing: Vec<float> // Each split spacing is relative to the one before it
		children: Vec<SplitBox> // If `n` is the length of `split_spacing`, the length of this is `n + 1`
	}
	```

	With that, having some type of window boundary would be neat

	Perhaps make the root nodes non-alternating with a normal KD-tree
	That might work
	I would have to draw out an example for that

	Maybe a K-D-B tree is the solution?
	*/

	pub children: Option<Vec<Self>>
}

impl Window {
	pub fn new(
		updater: WindowUpdater,
		state: dynamic_optional::DynamicOptional,
		contents: WindowContents,
		top_left: Vec2f, bottom_right: Vec2f,
		children: Option<Vec<Self>>) -> Self {

		std::assert!(top_left.is_left_of(bottom_right));

		let none_if_children_vec_is_empty = match &children {
			Some(inner_children) => {if inner_children.is_empty() {None} else {children}},
			None => None
		};

		Self {
			updater, state, contents, top_left, bottom_right,
			children: none_if_children_vec_is_empty
		}
	}
}

// TODO: put the unchanging params behind a common reference
pub fn render_windows_recursively(
	window: &mut Window,
	texture_pool: &mut texture::TexturePool,
	canvas: &mut CanvasSDL,
	parent_rect_in_pixels: sdl2::rect::Rect)

	-> GenericResult<()> {

	////////// Updating the window content first

	if let Some(updater) = window.updater {
		updater(window, texture_pool)?;
	}

	////////// Getting the new pixel-space bounding box for this window

	let parent_size_in_pixels = (
		parent_rect_in_pixels.width(), parent_rect_in_pixels.height()
	);

	let relative_window_size = window.bottom_right - window.top_left;

	let absolute_window_size_in_pixels = sdl2::rect::Rect::new(
		(window.top_left.x() * parent_size_in_pixels.0 as f32) as i32 + parent_rect_in_pixels.x(),
		(window.top_left.y() * parent_size_in_pixels.1 as f32) as i32 + parent_rect_in_pixels.y(),
		(relative_window_size.x() * parent_size_in_pixels.0 as f32) as u32,
		(relative_window_size.y() * parent_size_in_pixels.1 as f32) as u32,
	);

	////////// Handling different window content types

	match &window.contents {
		WindowContents::Color(color) => {
			use sdl2::render::BlendMode;

			let use_blending = color.a != 255 && canvas.blend_mode() != BlendMode::Blend;

			// TODO: make this state transition more efficient
			if use_blending {canvas.set_blend_mode(BlendMode::Blend);}
				canvas.set_draw_color(color.clone());
				canvas.fill_rect(absolute_window_size_in_pixels)?;
			if use_blending {canvas.set_blend_mode(BlendMode::None);}

		},

		WindowContents::Texture(texture) => {
			texture_pool.draw_texture_to_canvas(texture, canvas, absolute_window_size_in_pixels)?;
		}
	};

	////////// Updating all child windows

	if let Some(children) = &mut window.children {
		for child in children {
			render_windows_recursively(child, texture_pool, canvas, absolute_window_size_in_pixels)?;
		}
	}

	Ok(())

}
