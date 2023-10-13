use sdl2;

use crate::vec2f::{assert_in_unit_interval, Vec2f};
use crate::texture;
use crate::dynamic_optional;
use crate::generic_result::GenericResult;

////////// These are some general utilities

pub type ColorSDL = sdl2::pixels::Color;
pub type CanvasSDL = sdl2::render::Canvas<sdl2::video::Window>;

type HierarchalWindowUpdater = Option
	<fn(&mut HierarchalWindow, &mut texture::TexturePool)
	-> GenericResult<()>>;

//////////

pub enum WindowContents {
	PlainColor(ColorSDL),
	Texture(texture::TextureHandle)
}

impl WindowContents {
	pub fn make_color(r: u8, g: u8, b: u8) -> WindowContents {
		return WindowContents::PlainColor(ColorSDL::RGB(r, g, b));
	}

	// `a` ranges from 0 to 1
	pub fn make_transparent_color(r: u8, g: u8, b: u8, a: f32) -> WindowContents {
		assert_in_unit_interval(a);
		return WindowContents::PlainColor(ColorSDL::RGBA(r, g, b, (a * 255.0) as u8));
	}
}

pub struct HierarchalWindow {
	updater: HierarchalWindowUpdater,
	state: dynamic_optional::DynamicOptional,
	contents: WindowContents,
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

	children: Option<Vec<HierarchalWindow>>
}

impl HierarchalWindow {
	pub fn new(
		updater: HierarchalWindowUpdater,
		state: dynamic_optional::DynamicOptional,
		contents: WindowContents,
		top_left: Vec2f, bottom_right: Vec2f,
		children: Option<Vec<HierarchalWindow>>) -> HierarchalWindow {

		std::assert!(top_left.is_left_of(bottom_right));

		let none_if_children_vec_is_empty = match &children {
			Some(inner_children) => {if inner_children.is_empty() {None} else {children}},
			None => None
		};

		HierarchalWindow {
			updater, state, contents, top_left, bottom_right,
			children: none_if_children_vec_is_empty
		}
	}
}

// TODO: put the unchanging params behind a common reference
pub fn render_windows_recursively(
	window: &mut HierarchalWindow,
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
		WindowContents::PlainColor(color) => {
			use sdl2::render::BlendMode;

			let use_blending = color.a != 255 && canvas.blend_mode() != BlendMode::Blend;

			// TODO: make this state transition more efficient
			if use_blending {canvas.set_blend_mode(BlendMode::Blend);}
				canvas.set_draw_color(color.clone());
				canvas.fill_rect(absolute_window_size_in_pixels)?;
			if use_blending {canvas.set_blend_mode(BlendMode::None);}

		},

		WindowContents::Texture(texture) => {
			texture_pool.draw_texture_to_canvas(*texture, canvas, absolute_window_size_in_pixels)?;
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
