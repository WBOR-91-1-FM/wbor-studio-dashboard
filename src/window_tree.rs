use sdl2;

use crate::{
	texture,

	utility_types::{
		update_rate::{UpdateRate, FrameCounter},
		generic_result::GenericResult,
		dynamic_optional::DynamicOptional,
		vec2f::{assert_in_unit_interval, Vec2f},
	}
};

////////// These are some general utility types (TODO: put some of them in `utility_types`)

pub type ColorSDL = sdl2::pixels::Color;
pub type CanvasSDL = sdl2::render::Canvas<sdl2::video::Window>;

pub type PossibleWindowUpdater = Option<(
	fn(&mut Window, &mut texture::TexturePool, &DynamicOptional) -> GenericResult<()>,
	UpdateRate
)>;

pub type PossibleSharedWindowStateUpdater = Option<(
	fn(&mut DynamicOptional) -> GenericResult<()>,
	UpdateRate
)>;

// This data remains constant over a recursive rendering call
pub struct PerFrameConstantRenderingParams<'a> {
	pub sdl_canvas: CanvasSDL,
	pub texture_pool: texture::TexturePool<'a>,
	pub frame_counter: FrameCounter,
	pub shared_window_state: DynamicOptional,
	pub shared_window_state_updater: PossibleSharedWindowStateUpdater
}

//////////

pub enum WindowContents {
	Nothing,
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

//////////

pub struct Window {
	possible_updater: PossibleWindowUpdater,

	pub state: DynamicOptional,
	pub contents: WindowContents,

	// TODO: Make a fn to move a window in some direction (in a FPS-independent way)
	top_left: Vec2f,
	bottom_right: Vec2f,

	/* TODO: maybe do splitting here instead. Ideas for that:
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

	children: Option<Vec<Self>>
}

impl Window {
	pub fn new(
		possible_updater: PossibleWindowUpdater,
		state: DynamicOptional,
		contents: WindowContents,
		top_left: Vec2f, size: Vec2f,
		children: Option<Vec<Self>>) -> Self {

		let none_if_children_vec_is_empty = match &children {
			Some(inner_children) => {if inner_children.is_empty() {None} else {children}},
			None => None
		};

		Self {
			possible_updater, state, contents, top_left,
			bottom_right: top_left + size, children: none_if_children_vec_is_empty
		}
	}

	pub fn render_recursively(&mut self,
		rendering_params: &mut PerFrameConstantRenderingParams,
		sdl_parent_rect_size_in_pixels: sdl2::rect::Rect)

		-> GenericResult<()> {

		////////// Updating the window first

		/* TODO: if no updaters were called, then don't redraw anything
		(or if the updaters had no effect on the window).
		- Draw everything the first time around, without an updater.
		- The second time around + all other times, first check all the updaters.
		- If no updaters are called, don't redraw anything.
		- For any specific node, if that updater doesn't have an effect, then don't draw for that node. */

		let texture_pool = &mut rendering_params.texture_pool;

		if let Some((updater, update_rate)) = self.possible_updater {
			if update_rate.is_time_to_update(rendering_params.frame_counter) {
				updater(self, texture_pool, &rendering_params.shared_window_state)?;
			}
		}

		////////// Getting the new pixel-space bounding box for this window

		let sdl_parent_size_in_pixels = (
			sdl_parent_rect_size_in_pixels.width(), sdl_parent_rect_size_in_pixels.height()
		);

		let sdl_relative_window_size = self.bottom_right - self.top_left;

		let sdl_window_size_in_pixels = sdl2::rect::Rect::new(
			(self.top_left.x() * sdl_parent_size_in_pixels.0 as f32) as i32 + sdl_parent_rect_size_in_pixels.x(),
			(self.top_left.y() * sdl_parent_size_in_pixels.1 as f32) as i32 + sdl_parent_rect_size_in_pixels.y(),
			(sdl_relative_window_size.x() * sdl_parent_size_in_pixels.0 as f32) as u32,
			(sdl_relative_window_size.y() * sdl_parent_size_in_pixels.1 as f32) as u32,
		);

		////////// Handling different window content types

		let sdl_canvas = &mut rendering_params.sdl_canvas;

		match &self.contents {
			WindowContents::Nothing => {},

			WindowContents::Color(color) => {
				use sdl2::render::BlendMode;

				let use_blending = color.a != 255 && sdl_canvas.blend_mode() != BlendMode::Blend;

				// TODO: make this state transition more efficient
				if use_blending {sdl_canvas.set_blend_mode(BlendMode::Blend);}
					sdl_canvas.set_draw_color(color.clone());
					sdl_canvas.fill_rect(sdl_window_size_in_pixels)?;
				if use_blending {sdl_canvas.set_blend_mode(BlendMode::None);}

			},

			WindowContents::Texture(texture) => {
				texture_pool.draw_texture_to_canvas(texture, sdl_canvas, sdl_window_size_in_pixels)?;
			}
		};

		////////// Updating all child windows

		if let Some(children) = &mut self.children {
			for child in children {
				child.render_recursively(rendering_params, sdl_window_size_in_pixels)?;
			}
		}

		Ok(())
	}
}
