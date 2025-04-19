use sdl2::{
	self,
	rect::FRect,
	gfx::primitives::DrawRenderer
};

use crate::{
	texture::pool::{
		TexturePool,
		TextureHandle,
		TextureCreationInfo,
		RemakeTransitionInfo
	},

	utility_types::{
		vec2f::Vec2f,
		generic_result::*,
		dynamic_optional::DynamicOptional,
		update_rate::{FrameCounter, UpdateRate}
	}
};

////////// These are some general utility types

// TODO: maybe put these in `utility_types`
pub type ColorSDL = sdl2::pixels::Color;
pub type CanvasSDL = sdl2::render::Canvas<sdl2::video::Window>;

// This should be used instead of `FRect` whenever possible
#[derive(Copy, Clone)]
pub struct PreciseRect {
	pub x: f64,
	pub y: f64,
	pub width: f64,
	pub height: f64
}

impl PreciseRect {
	pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
		Self {x, y, width, height}
	}
}

impl From<PreciseRect> for FRect {
	fn from(rect: PreciseRect) -> FRect {
		FRect::new(rect.x as f32, rect.y as f32, rect.width as f32, rect.height as f32)
	}
}

pub struct WindowUpdaterParams<'a, 'b, 'c, 'd> {
	pub window: &'a mut WindowFieldsAccessibleToUpdater,
	pub texture_pool: &'b mut TexturePool<'c>,
	pub shared_window_state: &'d mut DynamicOptional,
	pub area_drawn_to_screen: (u32, u32)
}

pub type WindowUpdaters = Vec<(
	fn(WindowUpdaterParams) -> MaybeError,
	UpdateRate
)>;

// This data remains constant over a recursive rendering call
pub struct PerFrameConstantRenderingParams<'a> {
	pub draw_borders: bool,
	pub sdl_canvas: CanvasSDL,
	pub texture_pool: TexturePool<'a>,
	pub frame_counter: FrameCounter,
	pub shared_window_state: DynamicOptional
}

//////////

pub type GeneralLine<T> = (ColorSDL, Vec<T>);
pub type Line = GeneralLine<Vec2f>;

// TODO: make the border color a part of this
#[derive(Clone, PartialEq)]
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
		maybe_remake_transition_info: Option<&RemakeTransitionInfo>,
		get_fallback_texture_creation_info: fn() -> TextureCreationInfo<'static>) -> MaybeError {

		/* This is a macro for making or remaking a texture. If making or
		remaking fails, a fallback texture is put into that texture's slot. */
		macro_rules! try_to_make_or_remake_texture {
			($make_or_remake: expr, $make_or_remake_description: expr, $($extra_args:expr),*) => {{
				$make_or_remake(creation_info, $($extra_args),*).or_else(
					|failure_reason| {
						log::warn!("Unexpectedly failed while trying to {} texture, and reverting to a fallback \
							texture. Reason: '{failure_reason}'. Creation info: '{creation_info:?}'.", $make_or_remake_description);

						$make_or_remake(&get_fallback_texture_creation_info(), $($extra_args),*)
					}
				)
			}};
		}

		let updated_texture = if let Self::Texture(prev_texture) = self {
			if should_remake {
				try_to_make_or_remake_texture!(
					|a, b| texture_pool.remake_texture(a, b, maybe_remake_transition_info),
					"remake an existing",
					prev_texture
				)?
			}

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

// Maybe color and border radius
pub type WindowBorderInfo = Option<(ColorSDL, i16)>;

/* These fields are kept separate from the main `Window` type
to avoid the issue of iterating over the updaters, while passing
the fields in `WindowFieldsAccessibleToUpdater` mutably to updaters (this causes a borrow error).
The earlier fix to this problem was to clone the updaters every time while iterating, but that's
really inefficient, so making the field categories disjoint fixed the issue. */
pub struct WindowFieldsAccessibleToUpdater {
	state: DynamicOptional,
	contents: WindowContents,

	skip_drawing: bool,

	/* Note that if this is set, aspect ratio correction won't happen,
	except for 2 cases: colors and text textures, in which aspect ratio
	correction will never happen. */
	skip_aspect_ratio_correction: bool,

	border_info: WindowBorderInfo,

	// TODO: Make a fn to move a window in some direction (in a FPS-independent way)
	top_left: Vec2f,
	size: Vec2f,

	children: Vec<Window>
}

impl WindowFieldsAccessibleToUpdater {
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
}

pub struct Window {
	updaters: WindowUpdaters,
	fields_for_updater: WindowFieldsAccessibleToUpdater
}

impl Window {
	pub fn new(
		updaters: WindowUpdaters,
		state: DynamicOptional,
		contents: WindowContents,
		border_info: WindowBorderInfo,
		top_left: Vec2f, size: Vec2f,
		children: Vec<Self>) -> Self {

		// This is done to invoke a panic if the bottom right goes out of bounds
		let _bottom_right = top_left + size;

		Self {
			updaters,

			fields_for_updater: WindowFieldsAccessibleToUpdater {
				state,
				contents,
				skip_drawing: false,
				skip_aspect_ratio_correction: false,
				border_info,
				top_left,
				size,
				children
			}
		}
	}

	////////// These are some setters

	pub fn set_draw_skipping(&mut self, skip_drawing: bool) {
		self.fields_for_updater.set_draw_skipping(skip_drawing);
	}

	pub fn set_aspect_ratio_correction_skipping(&mut self, skip_aspect_ratio_correction: bool) {
		self.fields_for_updater.set_aspect_ratio_correction_skipping(skip_aspect_ratio_correction);
	}

	////////// These are the window rendering functions (both public and private)

	pub fn render(&mut self, rendering_params: &mut PerFrameConstantRenderingParams) {
		match rendering_params.sdl_canvas.output_size() {
			Ok(output_size) => {
				let sdl_window_bounds = PreciseRect::new(0.0, 0.0, output_size.0 as f64, output_size.1 as f64);
				self.inner_render(rendering_params, sdl_window_bounds);
			}
			Err(err) => {
				log::error!("Skipping rendering; could not get the canvas's output size. Reason: '{err}'.");
			}
		};
	}

	fn transform_vec2_to_parent_scale(v: Vec2f, parent_rect: PreciseRect) -> (f64, f64) {
		(v.x() * parent_rect.width + parent_rect.x, v.y() * parent_rect.height + parent_rect.y)
	}

	fn inner_render(&mut self,
		rendering_params: &mut PerFrameConstantRenderingParams,
		parent_rect: PreciseRect) {

		////////// Getting the new pixel-space bounding box for this window

		let fields = &self.fields_for_updater;
		let rect_origin = Self::transform_vec2_to_parent_scale(fields.top_left, parent_rect);

		let screen_dest = PreciseRect::new(
			rect_origin.0,
			rect_origin.1,
			fields.size.x() * parent_rect.width,
			fields.size.y() * parent_rect.height
		);

		////////// Updating the window

		/* TODO: if no updaters were called, then don't redraw anything
		(or if the updaters had no effect on the window).
		- Draw everything the first time around, without an updater.
		- The second time around + all other times, first check all the updaters.
		- If no updaters are called, don't redraw anything.
		- For any specific node, if that updater doesn't have an effect, then don't draw for that node. */

		for (updater, update_rate) in &self.updaters {
			if update_rate.is_time_to_update(rendering_params.frame_counter) {

				let params = WindowUpdaterParams {
					window: &mut self.fields_for_updater,
					texture_pool: &mut rendering_params.texture_pool,
					shared_window_state: &mut rendering_params.shared_window_state,
					area_drawn_to_screen: (screen_dest.width.ceil() as u32, screen_dest.height.ceil() as u32)
				};

				// TODO: report this as an internal dashboard error too
				if let Err(err) = updater(params) {
					log::error!("An error occurred while updating a window: '{err}'.");
				}
			}
		}

		if !self.fields_for_updater.skip_drawing {
			// TODO: report this as an internal dashboard error too
			if let Err(err) = self.draw_window_contents(rendering_params, screen_dest) {
				log::error!("An error occurred while drawing a window's contents: '{err}'.");
			}
		}

		////////// Updating all child windows

		for child in &mut self.fields_for_updater.children {
			child.inner_render(rendering_params, screen_dest);
		}
	}

	fn draw_window_contents(&self,
		rendering_params: &mut PerFrameConstantRenderingParams,
		uncorrected_screen_dest: PreciseRect) -> MaybeError {

		//////////

		let fields = &self.fields_for_updater;

		draw_contents(
			&fields.contents, rendering_params,
			uncorrected_screen_dest,
			fields.skip_aspect_ratio_correction
		)?;

		if rendering_params.draw_borders {
			if let Some((border_color, border_radius)) = fields.border_info {
				let (x1, y1, x2, y2) = (
					uncorrected_screen_dest.x as i16,
					uncorrected_screen_dest.y as i16,
					(uncorrected_screen_dest.x + uncorrected_screen_dest.width) as i16,
					(uncorrected_screen_dest.y + uncorrected_screen_dest.height) as i16
				);

				//////////

				possibly_draw_with_transparency(border_color, &mut rendering_params.sdl_canvas,
					|canvas| {
						// TODO: can I somehow cut off objects drawn inside/outside the border?
						canvas.rounded_rectangle(x1, y1, x2, y2, border_radius, border_color).to_generic()

						// I did this before:
						// canvas.draw_frect(uncorrected_screen_dest.into()).to_generic()
					}
				)?;
			}
		}

		return Ok(());

		////////// A function for drawing the contents passed to it

		fn draw_contents(
			contents: &WindowContents,
			rendering_params: &mut PerFrameConstantRenderingParams,
			uncorrected_screen_dest: PreciseRect,
			skip_aspect_ratio_correction: bool) -> MaybeError {

			let maybe_corrected_screen_dest = maybe_correct_aspect_ratio(
				contents, uncorrected_screen_dest, &mut rendering_params.texture_pool,
				skip_aspect_ratio_correction
			);

			let sdl_canvas = &mut rendering_params.sdl_canvas;

			match contents {
				WindowContents::Nothing => {},

				WindowContents::Color(color) => possibly_draw_with_transparency(
					*color, sdl_canvas, |canvas| {
						canvas.fill_frect::<FRect>(uncorrected_screen_dest.into()).to_generic()
					})?,

				WindowContents::Lines(line_series) => {
					use sdl2::rect::FPoint as PointSDL;

					for series in line_series {
						let converted_series: Vec<PointSDL> = series.1.iter().map(|&point| {
							let xy = Window::transform_vec2_to_parent_scale(point, maybe_corrected_screen_dest);
							PointSDL::new(xy.0 as f32, xy.1 as f32)
						}).collect();

						possibly_draw_with_transparency(series.0, sdl_canvas, |canvas|
							canvas.draw_flines(&*converted_series).to_generic()
						)?;
					}
				},

				/* TODO: eliminate the partially black border around
				the opaque areas of textures with alpha values */
				WindowContents::Texture(texture) =>
					rendering_params.texture_pool.draw_texture_to_canvas(
						texture, sdl_canvas, maybe_corrected_screen_dest
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

		fn possibly_draw_with_transparency(color: ColorSDL, sdl_canvas: &mut CanvasSDL,
			mut drawer: impl FnMut(&mut CanvasSDL) -> MaybeError) -> MaybeError {

			use sdl2::render::BlendMode;

			let use_blending = color.a != 255 && sdl_canvas.blend_mode() != BlendMode::Blend;

			// TODO: make this state transition more efficient
			if use_blending {sdl_canvas.set_blend_mode(BlendMode::Blend);}
				sdl_canvas.set_draw_color(color);
				drawer(sdl_canvas)?;
			if use_blending {sdl_canvas.set_blend_mode(BlendMode::None);}

			Ok(())
		}

		////////// A function for correcting the aspect ratio of some window contents

		fn maybe_correct_aspect_ratio(contents: &WindowContents,
			uncorrected_screen_dest: PreciseRect, texture_pool: &mut TexturePool,
			skip_aspect_ratio_correction: bool) -> PreciseRect {

			if skip_aspect_ratio_correction {
				uncorrected_screen_dest
			}
			else {
				match contents {
					WindowContents::Color(_) | WindowContents::Many(_) => uncorrected_screen_dest,

					WindowContents::Texture(texture) =>
						texture_pool.get_screen_draw_area_for_texture(texture, uncorrected_screen_dest, get_centered_subrect_with_aspect_ratio),

					_ => get_centered_subrect_with_aspect_ratio(uncorrected_screen_dest, 1.0)
				}
			}
		}

		////////// A function for making a rect within another one with a given aspect ratio

		fn get_centered_subrect_with_aspect_ratio(orig_rect: PreciseRect, desired_aspect_ratio: f64) -> PreciseRect {
			let (orig_w, orig_h) = (orig_rect.width, orig_rect.height);
			let orig_aspect_ratio = orig_w / orig_h;

			let (w, h) = if desired_aspect_ratio > orig_aspect_ratio {
				(orig_w, orig_w / desired_aspect_ratio)
			}
			else {
				(orig_h * desired_aspect_ratio, orig_h)
			};

			PreciseRect::new(
				orig_rect.x + (orig_w - w) * 0.5,
				orig_rect.y + (orig_h - h) * 0.5,
				w,
				h
			)
		}
	}
}
