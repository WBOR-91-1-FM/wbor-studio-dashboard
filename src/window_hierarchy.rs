use sdl2;
use crate::texture;
use crate::dynamic_optional;
use crate::generic_result::GenericResult;

////////// These are some general utilities

pub type ColorSDL = sdl2::pixels::Color;
pub type CanvasSDL = sdl2::render::Canvas<sdl2::video::Window>;

type HierarchalWindowUpdater = Option
	<fn(&mut HierarchalWindow, &mut texture::TexturePool)
	-> GenericResult<()>>;

fn assert_in_unit_interval(f: f32) {
	std::assert!(f >= 0.0 && f <= 1.0);
}

// A 0-1 normalized floating-point vec2 (TODO: put this in its own file)
pub struct Vec2f {
	x: f32,
	y: f32
}

impl Vec2f {
	pub fn new(x: f32, y: f32) -> Vec2f {
		assert_in_unit_interval(x);
		assert_in_unit_interval(y);
		Vec2f {x, y}
	}
}

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

		std::assert!(top_left.x < bottom_right.x);
		std::assert!(top_left.y < bottom_right.y);

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
	parent_rect: sdl2::rect::Rect)

	-> GenericResult<()> {

	////////// Updating the window content first

	if let Some(updater) = window.updater {
		updater(window, texture_pool)?;
	}

	////////// Getting the new pixel-space bounding box for this window

	let parent_width = parent_rect.width();
	let parent_height = parent_rect.height();

	let origin_and_size = (
		window.top_left.x,
		window.top_left.y,
		window.bottom_right.x - window.top_left.x,
		window.bottom_right.y - window.top_left.y,
	);

	let rescaled_rect = sdl2::rect::Rect::new(
		(origin_and_size.0 * parent_width as f32) as i32 + parent_rect.x(),
		(origin_and_size.1 * parent_height as f32) as i32 + parent_rect.y(),
		(origin_and_size.2 * parent_width as f32) as u32,
		(origin_and_size.3 * parent_height as f32) as u32,
	);

	////////// Handling different window content types

	// TODO: catch every error for each match branch
	match &window.contents {
		WindowContents::PlainColor(color) => {
			use sdl2::render::BlendMode;

			let use_blending = color.a != 255 && canvas.blend_mode() != BlendMode::Blend;

			// TODO: make this state transition more efficient
			if use_blending {canvas.set_blend_mode(BlendMode::Blend);}
				canvas.set_draw_color(color.clone());
				canvas.fill_rect(rescaled_rect)?;
			if use_blending {canvas.set_blend_mode(BlendMode::None);}

		},

		WindowContents::Texture(texture) => {
			texture_pool.draw_texture_to_canvas(*texture, canvas, rescaled_rect)?;
		}
	};

	////////// Updating all child windows

	if let Some(children) = &mut window.children {
		for child in children {
			render_windows_recursively(child, texture_pool, canvas, rescaled_rect)?;
		}
	}

	Ok(())

}
