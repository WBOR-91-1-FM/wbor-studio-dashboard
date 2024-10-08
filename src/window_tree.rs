use sdl2::{self, rect::Rect};

use crate::{
	utility_types::{
		vec2f::Vec2f,
		generic_result::*,
		dynamic_optional::DynamicOptional,
		update_rate::{UpdateRate, FrameCounter}
	},

	texture::{TexturePool, TextureHandle, TextureCreationInfo}
};

////////// These are some general utility types

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

// TODO: maybe put these in `utility_types`
pub type ColorSDL = sdl2::pixels::Color;
pub type CanvasSDL = sdl2::render::Canvas<sdl2::video::Window>;

/* TODO: can I pass a current time parameter in here,
in order to allow for timing-based effects like texture fade-in? */
pub struct WindowUpdaterParams<'a, 'b, 'c, 'd> {
	pub window: &'a mut Window,
	pub texture_pool: &'b mut TexturePool<'c>,
	pub shared_window_state: &'d mut DynamicOptional,
	pub area_drawn_to_screen: (u32, u32)
}

pub type PossibleWindowUpdater = Option<(
	fn(WindowUpdaterParams) -> MaybeError,
	UpdateRate
)>;

// This data remains constant over a recursive rendering call (TODO: make a constructor for this)
pub struct PerFrameConstantRenderingParams<'a> {
	pub sdl_canvas: CanvasSDL,
	pub texture_pool: TexturePool<'a>,
	pub frame_counter: FrameCounter,
	pub shared_window_state: DynamicOptional
}

//////////

pub type GeneralLine<T> = (ColorSDL, Vec<T>);
pub type Line = GeneralLine<Vec2f>;

// TODO: make the border color a part of this
#[derive(Clone)]
pub enum WindowContents {
	Nothing,
	Color(ColorSDL),
	Lines(Vec<Line>),
	Texture(TextureHandle),
	Many(Vec<WindowContents>) // Note: recursive `Many` items here are allowed.
}

impl WindowContents {
	pub fn make_texture_contents(creation_info: &TextureCreationInfo, texture_pool: &mut TexturePool) -> GenericResult<Self> {
		Ok(Self::Texture(texture_pool.make_texture(creation_info)?))
	}

	/* This is used for updating the texture of a window whose
	contents is a texture (but maybe starts out as something else) */
	pub fn update_as_texture(
		&mut self,
		should_remake: bool,
		texture_pool: &mut TexturePool,
		creation_info: &TextureCreationInfo,
		get_fallback_texture_creation_info: fn() -> TextureCreationInfo<'static>) -> MaybeError {

		/* This is a macro for making or remaking a texture. If making or
		remaking fails, a fallback texture is put into that texture's slot. */
		macro_rules! try_to_make_or_remake_texture {
			($make_or_remake: expr, $make_or_remake_description: expr, $($extra_args:expr),*) => {{
				$make_or_remake(creation_info, $($extra_args),*).or_else(
					|failure_reason| {
						log::warn!("Unexpectedly failed while trying to {} texture, and reverting to a fallback \
							texture. Reason: '{failure_reason}'.", $make_or_remake_description);

						$make_or_remake(&get_fallback_texture_creation_info(), $($extra_args),*)
					}
				)
			}};
		}

		let updated_texture = if let Self::Texture(prev_texture) = self {
			if should_remake {try_to_make_or_remake_texture!(|a, b| texture_pool.remake_texture(a, b), "remake an existing", prev_texture)?}
			prev_texture.clone()
		}
		else {
			/* There was not a texture before, and there's an initial one available now,
			so a first texture is being made. This should only happen once, at the program's
			start; otherwise, an unbound amount of new textures will be made. */
			try_to_make_or_remake_texture!(|a| texture_pool.make_texture(a), "make a new",)?
		};

		*self = Self::Texture(updated_texture);
		Ok(())
	}
}

//////////

pub struct Window {
	possible_updater: PossibleWindowUpdater,
	state: DynamicOptional,
	contents: WindowContents,

	skip_drawing: bool,

	/* Note that if this is set, aspect ratio correction won't happen,
	except for 2 cases: colors and text textures, in which aspect ratio
	correction will never happen. */
	skip_aspect_ratio_correction: bool,

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
			skip_aspect_ratio_correction: false,
			maybe_border_color,
			top_left, size,
			children: none_if_children_vec_is_empty
		}
	}

	////////// Some getters and setters

	pub fn get_state<T: 'static>(&self) -> &T {
		self.state.get()
	}

	pub fn get_state_mut<T: 'static>(&mut self) -> &mut T {
		self.state.get_mut()
	}

	pub const fn get_contents(&self) -> &WindowContents {
		&self.contents
	}

	pub fn get_contents_mut(&mut self) -> &mut WindowContents {
		&mut self.contents
	}

	pub fn set_draw_skipping(&mut self, skip_drawing: bool) {
		self.skip_drawing = skip_drawing;
	}

	pub const fn drawing_is_skipped(&self) -> bool {
		self.skip_drawing
	}

	pub fn set_aspect_ratio_correction_skipping(&mut self, skip_aspect_ratio_correction: bool) {
		self.skip_aspect_ratio_correction = skip_aspect_ratio_correction;
	}

	////////// These are the window rendering functions (both public and private)

	pub fn render(&mut self, rendering_params: &mut PerFrameConstantRenderingParams) -> MaybeError {
		let output_size = rendering_params.sdl_canvas.output_size().to_generic()?;
		let sdl_window_bounds = FRect {x: 0.0, y: 0.0, width: output_size.0 as f32, height: output_size.1 as f32};
		self.inner_render(rendering_params, sdl_window_bounds)
	}

	fn transform_vec2_to_parent_scale(v: Vec2f, parent_rect: FRect) -> (f32, f32) {
		(v.x() * parent_rect.width + parent_rect.x, v.y() * parent_rect.height + parent_rect.y)
	}

	fn inner_render(&mut self,
		rendering_params: &mut PerFrameConstantRenderingParams,
		parent_rect: FRect) -> MaybeError {

		////////// Getting the new pixel-space bounding box for this window

		let rect_origin = Self::transform_vec2_to_parent_scale(self.top_left, parent_rect);

		let screen_dest = FRect {
			x: rect_origin.0,
			y: rect_origin.1,
			width: self.size.x() * parent_rect.width,
			height: self.size.y() * parent_rect.height
		};

		////////// Updating the window

		/* TODO: if no updaters were called, then don't redraw anything
		(or if the updaters had no effect on the window).
		- Draw everything the first time around, without an updater.
		- The second time around + all other times, first check all the updaters.
		- If no updaters are called, don't redraw anything.
		- For any specific node, if that updater doesn't have an effect, then don't draw for that node. */

		if let Some((updater, update_rate)) = self.possible_updater {
			if update_rate.is_time_to_update(rendering_params.frame_counter) {
				updater(WindowUpdaterParams {
					window: self,
					texture_pool: &mut rendering_params.texture_pool,
					shared_window_state: &mut rendering_params.shared_window_state,
					area_drawn_to_screen: (screen_dest.width as u32, screen_dest.height as u32)
				})?;
			}
		}

		if !self.skip_drawing {
			self.draw_window_contents(rendering_params, screen_dest)?;
		}

		////////// Updating all child windows

		if let Some(children) = &mut self.children {
			for child in children {
				child.inner_render(rendering_params, screen_dest)?;
			}
		}

		Ok(())
	}

	fn draw_window_contents(&mut self,
		rendering_params: &mut PerFrameConstantRenderingParams,
		uncorrected_screen_dest: FRect) -> MaybeError {

		//////////

		draw_contents(
			&self.contents, rendering_params,
			uncorrected_screen_dest,
			self.skip_aspect_ratio_correction
		)?;

		if let Some(border_color) = &self.maybe_border_color {
			possibly_draw_with_transparency(border_color, &mut rendering_params.sdl_canvas,
				|canvas| canvas.draw_rect(uncorrected_screen_dest.into()).to_generic())?;
		}

		return Ok(());

		////////// A function for drawing the contents passed to it

		fn draw_contents(
			contents: &WindowContents,
			rendering_params: &mut PerFrameConstantRenderingParams,
			uncorrected_screen_dest: FRect,
			skip_aspect_ratio_correction: bool) -> MaybeError {

			let maybe_corrected_screen_dest = maybe_correct_aspect_ratio(
				contents, uncorrected_screen_dest, &rendering_params.texture_pool,
				skip_aspect_ratio_correction);

			let sdl_canvas = &mut rendering_params.sdl_canvas;

			match contents {
				WindowContents::Nothing => {},

				WindowContents::Color(color) => possibly_draw_with_transparency(
					color, sdl_canvas, |canvas|
						canvas.fill_rect::<Rect>(uncorrected_screen_dest.into()).to_generic()
					)?,

				WindowContents::Lines(line_series) => {
					use sdl2::rect::Point as PointSDL;

					for series in line_series {
						let converted_series: Vec<PointSDL> = series.1.iter().map(|&point| {
							let xy = Window::transform_vec2_to_parent_scale(point, maybe_corrected_screen_dest);
							PointSDL::new(xy.0 as i32, xy.1 as i32)
						}).collect();

						possibly_draw_with_transparency(&series.0, sdl_canvas, |canvas|
							canvas.draw_lines(&*converted_series).to_generic()
						)?;
					}
				},

				/* TODO: eliminate the partially black border around
				the opaque areas of textures with alpha values */
				WindowContents::Texture(texture) =>
					rendering_params.texture_pool.draw_texture_to_canvas(
						texture, sdl_canvas, maybe_corrected_screen_dest.into()
					)?,

				WindowContents::Many(many) => {
					for nested_contents in many {
						draw_contents(
							nested_contents, rendering_params,
							uncorrected_screen_dest,
							skip_aspect_ratio_correction
						)?;
					}
				}
			};

			Ok(())
		}

		////////// A function for drawing colors with transparency

		fn possibly_draw_with_transparency(color: &ColorSDL, sdl_canvas: &mut CanvasSDL,
			mut drawer: impl FnMut(&mut CanvasSDL) -> MaybeError) -> MaybeError {

			use sdl2::render::BlendMode;

			let use_blending = color.a != 255 && sdl_canvas.blend_mode() != BlendMode::Blend;

			// TODO: make this state transition more efficient
			if use_blending {sdl_canvas.set_blend_mode(BlendMode::Blend);}
				sdl_canvas.set_draw_color(*color);
				drawer(sdl_canvas)?;
			if use_blending {sdl_canvas.set_blend_mode(BlendMode::None);}

			Ok(())
		}

		////////// A function for correcting the aspect ratio of some window contents

		fn maybe_correct_aspect_ratio(contents: &WindowContents,
			uncorrected_screen_dest: FRect, texture_pool: &TexturePool,
			skip_aspect_ratio_correction: bool) -> FRect {

			match contents {
				WindowContents::Texture(texture) => {
					if skip_aspect_ratio_correction || texture_pool.is_text_texture(texture) {
						uncorrected_screen_dest
					}
					else {
						let texture_aspect_ratio = texture_pool.get_aspect_ratio_for(texture);
						get_centered_subrect_with_aspect_ratio(uncorrected_screen_dest, texture_aspect_ratio)
					}
				},

				WindowContents::Color(_) | WindowContents::Many(_) => uncorrected_screen_dest,

				_ => {
					if skip_aspect_ratio_correction {uncorrected_screen_dest}
					else {get_centered_subrect_with_aspect_ratio(uncorrected_screen_dest, 1.0)}
				}
			}
		}

		////////// A function for making a rect within another one with a given aspect ratio

		fn get_centered_subrect_with_aspect_ratio(orig_rect: FRect, desired_aspect_ratio: f32) -> FRect {
			let orig_aspect_ratio = orig_rect.width / orig_rect.height;

			let (width, height) = if desired_aspect_ratio > orig_aspect_ratio {
				(orig_rect.width, orig_rect.width / desired_aspect_ratio)
			}
			else {
				(orig_rect.height * desired_aspect_ratio, orig_rect.height)
			};

			FRect {
				x: orig_rect.x + (orig_rect.width - width) * 0.5,
				y: orig_rect.y + (orig_rect.height - height) * 0.5,
				width,
				height
			}
		}
	}
}
