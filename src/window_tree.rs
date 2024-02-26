use sdl2::{self, rect::Rect};

use crate::{
	utility_types::{
		vec2f::Vec2f,
		generic_result::GenericResult,
		dynamic_optional::DynamicOptional,
		update_rate::{UpdateRate, FrameCounter}
	},

	texture::{TexturePool, TextureHandle, TextureCreationInfo}

};

////////// These are some general utility types (TODO: put some of them in `utility_types`)

/* TODO: make this more similar to `Rect`, in terms of operations.
Also make a constructor for this. */
#[derive(Copy, Clone)]
struct FRect {
	pub x: f32,
	pub y: f32,
	pub width: f32,
	pub height: f32
}

impl From<FRect> for Rect {
	fn from(r: FRect) -> Self {
		Rect::new(
			r.x as i32, r.y as i32,
			r.width as u32, r.height as u32
		)
	}
}

pub type ColorSDL = sdl2::pixels::Color;
pub type CanvasSDL = sdl2::render::Canvas<sdl2::video::Window>;

pub type WindowUpdaterParams<'a, 'b, 'c, 'd> = (
	&'a mut Window,
	&'b mut TexturePool<'c>,
	&'d DynamicOptional, // This is the state that is shared among windows
	Rect // The area on the screen that the window is drawn to
);

// TODO: genericize these two over one typedef

pub type PossibleWindowUpdater = Option<(
	fn(WindowUpdaterParams) -> GenericResult<()>,
	UpdateRate
)>;

pub type PossibleSharedWindowStateUpdater = Option<(
	fn(&mut DynamicOptional) -> GenericResult<()>,
	UpdateRate
)>;

// This data remains constant over a recursive rendering call (TODO: make a constructor for this)
pub struct PerFrameConstantRenderingParams<'a> {
	pub sdl_canvas: CanvasSDL,
	pub texture_pool: TexturePool<'a>,
	pub frame_counter: FrameCounter,
	pub shared_window_state: DynamicOptional,
	pub shared_window_state_updater: PossibleSharedWindowStateUpdater
}

//////////

// A color paired with a vec of interconnected line points

pub type GeneralLine<T> = (ColorSDL, Vec<T>);
pub type Line = (ColorSDL, Vec<Vec2f>);

pub enum WindowContents {
	Nothing,
	Color(ColorSDL),
	Lines(Vec<Line>),
	Texture(TextureHandle)
}

//////////

pub struct Window {
	possible_updater: PossibleWindowUpdater,
	state: DynamicOptional,
	contents: WindowContents,

	skip_drawing: bool,
	maybe_border_color: Option<ColorSDL>,

	// TODO: Make a fn to move a window in some direction (in a FPS-independent way)
	top_left: Vec2f,
	size: Vec2f,

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
		maybe_border_color: Option<ColorSDL>,
		top_left: Vec2f, size: Vec2f,
		children: Option<Vec<Self>>) -> Self {

		let _bottom_right = top_left + size;

		let none_if_children_vec_is_empty = match &children {
			Some(inner_children) => {if inner_children.is_empty() {None} else {children}},
			None => None
		};

		Self {
			possible_updater, state, contents,
			skip_drawing: false,
			maybe_border_color,
			top_left, size,
			children: none_if_children_vec_is_empty
		}
	}

	////////// Some getters and setters

	pub fn get_state<T: 'static>(&self) -> &T {
		self.state.get_inner_value()
	}

	pub fn get_state_mut<T: 'static>(&mut self) -> &mut T {
		self.state.get_inner_value_mut()
	}

	pub fn get_contents_mut(&mut self) -> &mut WindowContents {
		&mut self.contents
	}

	pub fn drawing_is_skipped(&self) -> bool {
		self.skip_drawing
	}

	pub fn set_draw_skipping(&mut self, skip_drawing: bool) {
		self.skip_drawing = skip_drawing;
	}

	////////// This is used for updating the texture of a window whose contents is a texture (but starts out as nothing)

	pub fn update_texture_contents(
		&mut self,
		should_remake: bool,
		texture_pool: &mut TexturePool,
		texture_creation_info: &TextureCreationInfo,
		fallback_texture_creation_info: &TextureCreationInfo) -> GenericResult<()> {

		/* This is a macro for making or remaking a texture. If making or
		remaking fails, a fallback texture is put into that texture's slot. */
		macro_rules! try_to_make_or_remake_texture {
			($make_or_remake: expr, $make_or_remake_description: expr, $($extra_args:expr),*) => {{
				$make_or_remake(texture_creation_info, $($extra_args),*).or_else(
					|failure_reason| {
						println!("Unexpectedly failed while trying to {} texture, and reverting to a fallback \
							texture. Reason: '{}'.", $make_or_remake_description, failure_reason);

						$make_or_remake(fallback_texture_creation_info, $($extra_args),*)
					}
				)
			}};
		}

		let updated_texture = if let WindowContents::Texture(prev_texture) = &self.contents {
			if should_remake {try_to_make_or_remake_texture!(|a, b| texture_pool.remake_texture(a, b), "remake an existing", prev_texture)?}
			prev_texture.clone()
		}
		else {
			/* There was not a texture before, and there's an initial one available now,
			so a first texture is being made. This should only happen once, at the program's
			start; otherwise, an unbound amount of new textures will be made. */
			try_to_make_or_remake_texture!(|a| texture_pool.make_texture(a), "make a new",)?
		};

		self.contents = WindowContents::Texture(updated_texture);
		Ok(())
	}

	////////// These are the window rendering functions (both public and private)

	pub fn render(&mut self, rendering_params: &mut PerFrameConstantRenderingParams) -> GenericResult<()> {
		let output_size = rendering_params.sdl_canvas.output_size()?;
		let sdl_window_bounds = FRect {x: 0.0, y: 0.0, width: output_size.0 as f32, height: output_size.1 as f32};
		self.inner_render(rendering_params, sdl_window_bounds)
	}

	fn transform_vec2_to_parent_scale(v: Vec2f, parent_rect: FRect) -> (f32, f32) {
		(v.x() * parent_rect.width + parent_rect.x, v.y() * parent_rect.height + parent_rect.y)
	}

	fn inner_render(&mut self,
		rendering_params: &mut PerFrameConstantRenderingParams,
		parent_rect: FRect) -> GenericResult<()> {

		////////// Getting the new pixel-space bounding box for this window

		let rect_origin = Self::transform_vec2_to_parent_scale(self.top_left, parent_rect);

		let rect_in_pixels = FRect {
			x: rect_origin.0,
			y: rect_origin.1,
			width: self.size.x() * parent_rect.width,
			height: self.size.y() * parent_rect.height
		};

		let rect_in_pixels_sdl: Rect = rect_in_pixels.into();

		////////// Updating the window

		/* TODO: if no updaters were called, then don't redraw anything
		(or if the updaters had no effect on the window).
		- Draw everything the first time around, without an updater.
		- The second time around + all other times, first check all the updaters.
		- If no updaters are called, don't redraw anything.
		- For any specific node, if that updater doesn't have an effect, then don't draw for that node. */

		if let Some((updater, update_rate)) = self.possible_updater {
			if update_rate.is_time_to_update(rendering_params.frame_counter) {
				updater((self, &mut rendering_params.texture_pool, &rendering_params.shared_window_state, rect_in_pixels_sdl))?;
			}
		}

		if !self.skip_drawing {
			self.draw_window_contents(rendering_params, rect_in_pixels, rect_in_pixels_sdl)?;
		}

		////////// Updating all child windows

		if let Some(children) = &mut self.children {
			for child in children {
				child.inner_render(rendering_params, rect_in_pixels)?;
			}
		}

		Ok(())
	}

	fn draw_window_contents(
		&mut self,
		rendering_params: &mut PerFrameConstantRenderingParams,
		screen_dest: FRect, screen_dest_sdl: Rect) -> GenericResult<()> {

		////////// A function for drawing colors with transparency

		fn possibly_draw_with_transparency(color: &ColorSDL,
			sdl_canvas: &mut CanvasSDL, mut drawer: impl FnMut(&mut CanvasSDL) -> GenericResult<()>)
			-> GenericResult<()> {

			use sdl2::render::BlendMode;

			let use_blending = color.a != 255 && sdl_canvas.blend_mode() != BlendMode::Blend;

			// TODO: make this state transition more efficient
			if use_blending {sdl_canvas.set_blend_mode(BlendMode::Blend);}
				sdl_canvas.set_draw_color(*color);
				drawer(sdl_canvas)?;
			if use_blending {sdl_canvas.set_blend_mode(BlendMode::None);}

			Ok(())
		}

		//////////

		let sdl_canvas = &mut rendering_params.sdl_canvas;

		match &self.contents {
			WindowContents::Nothing => {},

			WindowContents::Color(color) => {
				possibly_draw_with_transparency(color, sdl_canvas, |canvas| Ok(canvas.fill_rect(screen_dest_sdl)?))?;
			},

			WindowContents::Lines(line_series) => {
				use sdl2::rect::Point as PointSDL;

				for series in line_series {
					let converted_series: Vec<PointSDL> = series.1.iter().map(|&point| {
						let xy = Self::transform_vec2_to_parent_scale(point, screen_dest);
						PointSDL::new(xy.0 as i32, xy.1 as i32)
					}).collect();

					possibly_draw_with_transparency(&series.0, sdl_canvas, |canvas| {
						canvas.draw_lines(&*converted_series)?;
						Ok(())
					})?;
				}
			},

			/* TODO: eliminate the partially black border around
			the opaque areas of textures with alpha values */
			WindowContents::Texture(texture) => {
				rendering_params.texture_pool.draw_texture_to_canvas(texture, sdl_canvas, screen_dest_sdl)?;
			}
		};

		if let Some(border_color) = &self.maybe_border_color {
			possibly_draw_with_transparency(border_color, sdl_canvas, |canvas| Ok(canvas.draw_rect(screen_dest_sdl)?))?;
		}

		Ok(())
	}
}
