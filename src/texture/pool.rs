use std::{
	borrow::Cow,
	collections::{HashMap, VecDeque}
};

use sdl2::{
	ttf,
	surface::Surface,
	image::LoadTexture,
	rect::{Rect, FRect},
	render::{self, BlendMode, Texture}
};

use crate::{
	texture::text,
	window_tree::{CanvasSDL, PixelAreaSDL, PreciseRect},

	utils::{
		time::*,
		request,
		file_utils,
		generic_result::*,
		vec2f::assert_in_unit_interval
	}
};

//////////

/* Input: the percent done with the transition. Domain: [0, 1).
Output: the opacities of the background and foreground textures. Range: [0, 1]. */
pub type TextureTransitionOpacityEaser = fn(f64) -> (f64, f64);

/* Under texture transitions, textures will stretch or shrink to fit into
the new texture's size and shape. The amount shrunken or stretched depends on
the percent done with the transition. This function determines how the aspect ratio
will change during the transition, either linearly, or via some easing function.

Input: the percent done with the transition. Domain: [0, 1].
Output: a new percent used to ease the aspect ratio at a different rate. Range: [0, 1]. */
pub type TextureTransitionAspectRatioEaser = fn(f64) -> f64;

#[derive(Clone)]
pub struct RemakeTransitionInfo {
	duration: Duration,
	opacity_easer: TextureTransitionOpacityEaser,
	aspect_ratio_easer: TextureTransitionAspectRatioEaser
}

impl RemakeTransitionInfo {
	pub fn new(duration: Duration, opacity_easer: TextureTransitionOpacityEaser, aspect_ratio_easer: TextureTransitionAspectRatioEaser) -> Self {
		Self {duration, opacity_easer, aspect_ratio_easer}
	}
}

//////////

// TODO: perhaps only only storing the creation info would ease memory usage?
struct RemakeTransition<'a> {
	new_texture: Texture<'a>,

	// The end time is not set until the transition starts (it might be waiting in a queue until then)
	end_time: Option<ReferenceTimestamp>,

	transition_info: RemakeTransitionInfo,
	maybe_text_metadata: Option<text::TextMetadataItem>
}

impl<'a> RemakeTransition<'a> {
	fn new(new_texture: Texture<'a>, transition_info: &RemakeTransitionInfo, creation_info: &TextureCreationInfo) -> Self {
		let maybe_text_metadata = text::TextMetadataItem::maybe_new(&new_texture, creation_info);

		Self {
			new_texture,
			end_time: None,
			transition_info: transition_info.clone(),
			maybe_text_metadata
		}
	}

	// This is the function used when the transition is first used. If over 1, it will be clamped to 1.
	fn get_percent_done(&mut self) -> f64 {
		let now = get_reference_time();

		if self.end_time.is_none() {
			// The end time is not set immediately, because the transition only starts once `get_percent_done` has been called
			self.end_time = Some(now + self.transition_info.duration);
		}

		let num_ms_left = (self.end_time.unwrap() - now).num_milliseconds();
		let total_time_for_transition = self.transition_info.duration.num_milliseconds();

		let percent = 1.0 - (num_ms_left as f64 / total_time_for_transition as f64);
		percent.min(1.0)
	}
}

//////////

struct RemakeTransitions<'a> {
	transitions: HashMap<TextureHandle, VecDeque<RemakeTransition<'a>>>,
	max_queue_size: usize
}

impl<'a> RemakeTransitions<'a> {
	fn new(max_queue_size: usize) -> Self {
		Self {transitions: HashMap::new(), max_queue_size}
	}

	fn get_from_handle(&self, handle: &TextureHandle) -> Option<&RemakeTransition> {
		self.transitions.get(handle).map(|queue| queue.front())?
	}

	fn get_from_handle_mut(&mut self, handle: &TextureHandle) -> Option<&mut RemakeTransition<'a>> {
		self.transitions.get_mut(handle).map(|queue| queue.front_mut())?
	}

	fn queue_new(&mut self, handle: &TextureHandle, mut transition: RemakeTransition<'a>) {
		// TODO: remove the blend mode if needed after the transition
		transition.new_texture.set_blend_mode(BlendMode::Blend);

		let queue = self.transitions
			.entry(handle.clone()).or_insert_with(|| VecDeque::with_capacity(1));

		if queue.len() > self.max_queue_size {
			log::warn!("Your texture queue is unusually large; in order to not use too much memory, discarding this queue entry.");
		}
		else {
			queue.push_back(transition);
		}

	}

	fn dequeue_current(&mut self, handle: &TextureHandle) -> Option<RemakeTransition<'a>> {
		self.transitions.get_mut(handle).and_then(|queue|
			queue.pop_front().map(|mut transition| {
				/* Setting the alpha mod to this, so that if the screen became unfocused,
				the alpha mod becomes set to its final value (instead of leaving it slightly translucent).
				TODO: perhaps set to a prev alpha mod, or a queued future one? */
				transition.new_texture.set_alpha_mod(255);
				transition
		}))
	}
}

//////////

// TODO: use `Cow` around the whole struct instead, if possible
#[derive(Hash, Debug)]
pub enum TextureCreationInfo<'a> {
	RawBytes(Cow<'a, [u8]>),
	Path(Cow<'a, str>),
	Url(Cow<'a, str>),
	Text((Cow<'a, text::FontInfo>, text::TextDisplayInfo<'a>))
}

impl TextureCreationInfo<'_> {
	fn raw_bytes(contents: Vec<u8>) -> GenericResult<Self> {
		Ok(TextureCreationInfo::RawBytes(Cow::Owned(contents)))
	}

	pub fn from_path(path: &str) -> TextureCreationInfo {
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

//////////

/* TODO:
- Later on, if I am using multiple texture pools,
add an id to each texture handle that is meant to match the pool
(to verify that the pool and the handle are only used together).
Otherwise, try to find some way to verify that it's a singleton.

- Will textures be destroyed when dropped currently, and if so, would using
the `unsafe_textures` feature help this?

- Split a lot of the code into distinct files
*/

pub struct TexturePool<'a> {
	max_texture_size: PixelAreaSDL,

	/* Used as an offset time in some time calculations,
	in contrast to the start of Unix time (just to not lose too much
	precision with big numbers). */
	init_time: ReferenceTimestamp,

	texture_creator: &'a TextureCreator,
	ttf_context: &'a ttf::Sdl2TtfContext,

	textures: Vec<Texture<'a>>,

	// This maps font paths and point sizes to fonts (TODO: should I limit the cache size?)
	font_cache: HashMap<FontCacheKey, FontPair<'a>>,

	// This maps texture handles of side-scrolling text textures to metadata about that scrolling text
	text_metadata_set: text::TextMetadataSet,

	// This maps texture handles to remake transitions (structs that define how textures should be rendered when remade)
	remake_transitions: RemakeTransitions<'a>
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
		max_texture_size: PixelAreaSDL,
		max_remake_transition_queue_size: usize) -> Self {

		Self {
			max_texture_size,
			init_time: get_reference_time(),
			texture_creator,
			ttf_context,

			textures: Vec::new(),
			font_cache: HashMap::new(),
			text_metadata_set: text::TextMetadataSet::new(),
			remake_transitions: RemakeTransitions::new(max_remake_transition_queue_size)
		}
	}

	//////////

	pub fn get_screen_draw_area_for_texture(&mut self, handle: &TextureHandle,
		uncorrected_screen_dest: PreciseRect,
		get_centered_subrect_with_aspect_ratio: fn(PreciseRect, f64) -> PreciseRect) -> PreciseRect {

		fn get_aspect_ratio(texture: &Texture) -> f64 {
			let query = texture.query();
			query.width as f64 / query.height as f64
		}

		fn lerp(a: f64, b: f64, t: f64) -> f64 {
			a + (b - a) * t
		}

		fn lerp_rects(a: PreciseRect, b: PreciseRect, t: f64) -> PreciseRect {
			PreciseRect::new(
				lerp(a.x, b.x, t),
				lerp(a.y, b.y, t),
				lerp(a.width, b.width, t),
				lerp(a.height, b.height, t)
			)
		}

		let curr_is_text_texture = self.text_metadata_set.contains_handle(handle);

		if let Some(transition) = self.remake_transitions.get_from_handle_mut(handle) {
			let percent_done = transition.get_percent_done();
			let eased_percent_done = (transition.transition_info.aspect_ratio_easer)(percent_done);
			assert_in_unit_interval(eased_percent_done);

			// If the next one is a text texture
			if transition.maybe_text_metadata.is_some() {
				if curr_is_text_texture {
					// Case 1: text -> text. No size change.
					uncorrected_screen_dest
				}
				else {
					// Case 2: normal -> text. Note: this is not aspect-ratio-corrected.

					let bg_aspect_ratio = get_aspect_ratio(self.get_texture_from_handle(handle));
					let normal_rect = get_centered_subrect_with_aspect_ratio(uncorrected_screen_dest, bg_aspect_ratio);
					let text_rect = uncorrected_screen_dest;
					lerp_rects(normal_rect, text_rect, eased_percent_done)
				}
			}
			else if curr_is_text_texture {
				// Case 3: text -> normal. Note: this is not aspect-ratio-corrected.

				let text_rect = uncorrected_screen_dest;
				let fg_aspect_ratio = get_aspect_ratio(&transition.new_texture);
				let normal_rect = get_centered_subrect_with_aspect_ratio(uncorrected_screen_dest, fg_aspect_ratio);
				lerp_rects(text_rect, normal_rect, eased_percent_done)
			}
			else {
				// Case 4: normal -> normal.

				// This one linearly interpolates the two aspect ratios, and then does AR correction. Intermediate rect is AR-corrected.
				let fg_aspect_ratio = get_aspect_ratio(&transition.new_texture);
				let bg_aspect_ratio = get_aspect_ratio(self.get_texture_from_handle(handle));
				let lerped_aspect_ratio = lerp(bg_aspect_ratio, fg_aspect_ratio, eased_percent_done);

				get_centered_subrect_with_aspect_ratio(uncorrected_screen_dest, lerped_aspect_ratio)

				/*
				// This one does a linear interpolation between two aspect-ratio-corrected rects. Intermediate rect is not AR-corrected.
				lerp_rects(
					get_centered_subrect_with_aspect_ratio(uncorrected_screen_dest, bg_aspect_ratio),
					get_centered_subrect_with_aspect_ratio(uncorrected_screen_dest, fg_aspect_ratio),
					eased_percent_done
				)
				*/
			}
		}
		else if curr_is_text_texture {
			// Case 5: just text.
			uncorrected_screen_dest
		}
		else {
			// Case 6: just normal.
			let bg_aspect_ratio = get_aspect_ratio(self.get_texture_from_handle(handle));
			get_centered_subrect_with_aspect_ratio(uncorrected_screen_dest, bg_aspect_ratio)
		}
	}

	/*
	pub fn size(&self) -> usize {
		self.textures.len()
	}
	*/

	pub fn draw_texture_to_canvas(&mut self, handle: &TextureHandle,
		canvas: &mut CanvasSDL, screen_dest: PreciseRect) -> MaybeError {

		// Using the integer coordinates here, since it's more compatible with the text rendering code (TODO: perhaps change in the future)
		let integer_screen_dest = Rect::new(
			screen_dest.x as i32,
			screen_dest.y as i32,
			screen_dest.width.ceil() as _,
			screen_dest.height.ceil() as _
		);

		// Note: the foreground is a transition layer, drawn on top of the base texture.
		let mut draw_helper =
			|this: &mut Self, maybe_opacity: Option<f64>, draw_foreground: bool| {

			////////// First task: determine if the texture should be drawn

			// If there's an opacity to modify, see if it's in range. Otherwise, draw by default.
			let should_draw = if let Some(opacity) = maybe_opacity {
				// Skip drawing if the opacity is less than or equal to zero, or greater than one
				let should_draw = opacity > 0.0 && opacity <= 1.0;

				if should_draw {

					// If drawing, first get a mutable texture
					let texture_mut = if draw_foreground {
						&mut this.remake_transitions.get_from_handle_mut(handle).unwrap().new_texture
					}
					else {
						this.get_texture_from_handle_mut(handle)
					};

					// Then, set the alpha mod accordingly
					let alpha = (opacity * 255.0) as u8;
					texture_mut.set_alpha_mod(alpha);
				}

				should_draw
			}
			else {
				true
			};

			////////// Second task: draw if necessary

			if should_draw {
				let (texture, maybe_text_metadata) = if draw_foreground {
					let transition = this.remake_transitions.get_from_handle(handle).unwrap();
					(&transition.new_texture, transition.maybe_text_metadata.as_ref())
				}
				else {
					let texture = this.get_texture_from_handle(handle);
					let text_metadata = this.text_metadata_set.get(handle);
					(texture, text_metadata)
				};

				if let Some(text_metadata) = maybe_text_metadata {
					this.draw_text_texture_to_canvas(texture, text_metadata, canvas, integer_screen_dest)
				}
				else {
					canvas.copy_f::<_, FRect>(texture, None, screen_dest.into()).to_generic_result()
				}
			}
			else {
				Ok(())
			}
		};

		//////////

		if let Some(transition) = self.remake_transitions.get_from_handle_mut(handle) {
			let percent_done = transition.get_percent_done();

			if percent_done == 1.0 { // Finishing the transition
				let moved_transition = self.remake_transitions.dequeue_current(handle).unwrap();

				// Carry over the old color mod (TODO: do this in another helper function, and carry over the alpha mod and blend mode too)
				/*
				let old_texture = self.get_texture_from_handle(handle);
				let prev_color_mod = old_texture.color_mod();
				moved_transition.new_texture.set_color_mod(prev_color_mod.0, prev_color_mod.1, prev_color_mod.2);
				*/

				// Update the text metadata appropriately
				self.text_metadata_set.update(handle, &moved_transition.maybe_text_metadata);

				// And finally, move the new texture into its new slot
				*self.get_texture_from_handle_mut(handle) = moved_transition.new_texture;
			}
			else {
				let (bg_opacity, fg_opacity) = (transition.transition_info.opacity_easer)(percent_done);
				assert_in_unit_interval(bg_opacity);
				assert_in_unit_interval(fg_opacity);

				let bg_texture = self.get_texture_from_handle_mut(handle);
				bg_texture.set_blend_mode(BlendMode::Blend); // TODO: only set once

				// Draw the background, and then the foreground
				draw_helper(self, Some(bg_opacity), false)?;
				draw_helper(self, Some(fg_opacity), true)?;

				return Ok(()); // Not continuing with any normal drawing here
			}
		}

		//////////

		// This invocation runs whenever there is no foreground transition layer to draw
		draw_helper(self, None, false)
	}

	fn draw_text_texture_to_canvas(
		&self, texture: &Texture,
		text_metadata: &text::TextMetadataItem,
		canvas: &mut CanvasSDL, screen_dest: Rect) -> MaybeError {

		// This can be extended later to allow for stuff like rotation
		fn draw(texture: &Texture, canvas: &mut CanvasSDL, src: Option<Rect>, dest: Rect) -> MaybeError {
			canvas.copy(texture, src, dest).to_generic_result()
		}

		// TODO: ensure that this works 100% of the time, using Kani
		fn compute_time_seed(secs_fract: f64, text_metadata: &text::TextMetadataItem) -> f64 {
			/* Note: any text that appears to scroll faster when compressed on the x-axis (during a transition)
			is not scrolling faster; it's just getting 'pushed' rightwards (or 'pulled' leftwards);
			so it is technically moving at a faster speed, but relative to the area of movement itself,
			it's not moving any faster. */

			let scroll_speed = text_metadata.scroll_speed;
			let unchanged_period = text_metadata.scroll_easer.1;
			let period = unchanged_period / scroll_speed;

			// I am modding the secs fract by the period, so that the time values don't get too large
			(secs_fract % period) * scroll_speed
		}

		//////////

		let texture_size = text_metadata.size;
		let dest_width = screen_dest.width();
		let time_since_start = get_reference_time().signed_duration_since(self.init_time);

		let secs_fract = time_since_start.num_milliseconds() as f64 / 1000.0;
		let time_seed = compute_time_seed(secs_fract, text_metadata);

		let (scroll_fract, should_wrap) = (text_metadata.scroll_easer.0)(
			time_seed, text_metadata.scroll_easer.1, texture_size.0 <= dest_width
		);

		assert_in_unit_interval(scroll_fract);

		//////////

		let mut x = texture_size.0;
		if !should_wrap {x -= dest_width;}

		//////////

		let texture_src = Rect::new(
			(x as f64 * scroll_fract) as i32,
			0, dest_width, texture_size.1
		);

		if !should_wrap {
			return draw(texture, canvas, Some(texture_src), screen_dest);
		}

		//////////

		let (right_screen_dest, possible_left_rects) = Self::split_overflowing_scrolled_rect(
			texture_src, screen_dest, texture_size, &text_metadata.text
		);

		draw(texture, canvas, Some(texture_src), right_screen_dest)?;

		if let Some((left_texture_src, left_screen_dest)) = possible_left_rects {
			draw(texture, canvas, Some(left_texture_src), left_screen_dest)?;
		}

		Ok(())
	}

	/* This returns the left/righthand screen dest, and a possible other texture
	src and screen dest that may wrap around to the left side of the screen */
	fn split_overflowing_scrolled_rect(
		texture_src: Rect, screen_dest: Rect,
		texture_size: PixelAreaSDL,
		text: &str) -> (Rect, Option<(Rect, Rect)>) {

		/* Input data notes:
		- `texture_src.width == screen_dest.width`
		- `texture_src.height` == `screen_dest.height`
		- `texture_src.width != texture_width` (`texture_src.width` will be smaller or equal)

		This only holds true when the texture is not part of a transition, though.
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


	//////////

	pub fn make_texture(&mut self, creation_info: &TextureCreationInfo) -> GenericResult<TextureHandle> {
		let handle = TextureHandle {handle: self.textures.len() as InnerTextureHandle};
		let texture = self.make_raw_texture(creation_info)?;

		self.text_metadata_set.update(&handle, &text::TextMetadataItem::maybe_new(&texture, creation_info));
		self.textures.push(texture);

		Ok(handle)
	}

	// TODO: if possible, update the texture in-place instead (if they occupy the amount of space, or less)
	pub fn remake_texture(&mut self, creation_info: &TextureCreationInfo, handle: &TextureHandle,
		maybe_remake_transition_info: Option<&RemakeTransitionInfo>) -> MaybeError {

		/* TODO: for remakes, defer the creation of this until a later point, if possible
		(otherwise, queueing many at once will be quite slow) */
		let new_texture = self.make_raw_texture(creation_info)?;

		if let Some(remake_transition_info) = maybe_remake_transition_info {
			self.remake_transitions.queue_new(handle, RemakeTransition::new(
				new_texture, remake_transition_info, creation_info
			));
		}
		else {
			self.text_metadata_set.update(handle, &text::TextMetadataItem::maybe_new(&new_texture, creation_info));
			*self.get_texture_from_handle_mut(handle) = new_texture;
		}

		Ok(())
	}

	// TODO: allow for texture deletion too

	////////// TODO: implement these fully

	/*
	pub fn set_color_mod_for(&mut self, _handle: &TextureHandle, _r: u8, _g: u8, _b: u8) {
		unimplemented!("Texture color mod setting is currently not supported! In the future, it will \
			be supported by carrying it over for transitioning/remade textures.");

		// self.get_texture_from_handle_mut(handle).set_color_mod(r, g, b);
	}

	pub fn set_alpha_mod_for(&mut self, _handle: &TextureHandle, _a: u8) {
		unimplemented!("Texture alpha mod setting is currently not supported! In the future, it will \
			be supported by using it as a start/end interpolant for transitioning textures, \
			and carrying it over for remade textures.");

		// self.get_texture_from_handle_mut(handle).set_alpha_mod(a);
	}
	*/

	pub fn set_blend_mode_for(&mut self, handle: &TextureHandle, _blend_mode: BlendMode) {
		// TODO: specify a blend mode when making a transition? Or perhaps just do queueing logic internally here?
		if self.remake_transitions.get_from_handle(handle).is_some() {
			unimplemented!("Cannot set the blend mode during a remake transition! In The future, it will \
				be supported by queueing the new blend mode for once the transition is done somehow \
				(not all blend modes may be valid for transitions).");
		}

		// self.get_texture_from_handle_mut(handle).set_blend_mode(blend_mode);
	}

	//////////

	fn get_texture_from_handle(&self, handle: &TextureHandle) -> &Texture<'a> {
		&self.textures[handle.handle as usize]
	}

	fn get_texture_from_handle_mut(&mut self, handle: &TextureHandle) -> &mut Texture<'a> {
		&mut self.textures[handle.handle as usize]
	}

	//////////

	fn get_font_pair(&mut self, key: FontCacheKey, maybe_options: Option<&text::FontInfo>) -> &FontPair {
		let fonts = self.font_cache.entry(key).or_insert_with( // TODO: should I use `or_insert_with_key` instead?
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
				font.set_hinting((*options.hinting).clone());

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
		text_display_info: &text::TextDisplayInfo) -> GenericResult<(FontPointSize, PixelAreaSDL)> {

		let initial_output_size = initial_font.size_of(text_display_info.text.inner())?;

		let height_ratio_from_expected_size = text_display_info.pixel_area.1 as f64 / initial_output_size.1 as f64;
		let adjusted_point_size = Self::INITIAL_POINT_SIZE as f64 * height_ratio_from_expected_size;

		// This seems to generally work better than `round` or `ceil` for the adjusted point size instead.
		Ok((adjusted_point_size as FontPointSize, initial_output_size))
	}

	//////////

	/* Assuming that the passed-in text will not result in a zero-width
	surface (that is handled in `make_text_surface`). */
	fn inner_make_text_surface(text_display_info: &text::TextDisplayInfo,
		font_pair: &FontPair, font_has_char: fn(&ttf::Font, char) -> bool,
		max_texture_width: u32) -> GenericResult<Surface<'a>> {

		let chars: Vec<char> = text_display_info.text.inner().chars().collect();
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

				let mut did_monospace_cutting = false;

				/* If the font is monospace (and not italicized) and it exceeds the
				max texture width, cut off enough characters to make it fit in one texture.
				I am not running this branch for italicized fonts since italicized fonts are
				not really monospaced per character. */
				if chosen_font.face_is_fixed_width() && !chosen_font.get_style().intersects(ttf::FontStyle::ITALIC) {
					log::debug!("Doing optimized monospace text span cutting");
					let orig_span_len = span.len();
					let first_char_pixel_width = chosen_font.size_of_char(span[0])?.0;

					//////////

					/* Checking that the monospace property holds (TODO: in the future, to guarantee this better, build up a set
					of all characters in the font that break the monospace property, and prefilter them out in `DisplayText`).
					Perhaps ignore control characters? Not sure how some of them would interfere with the monospace property... */
					let monospace_property_holds = first_char_pixel_width * orig_span_len as u32 == subsurface_width;

					if monospace_property_holds {
						let pixel_overstep = next_total_width - max_texture_width;
						let approx_char_overstep = pixel_overstep as f64 / subsurface_width as f64 * orig_span_len as f64;
						let char_overstep = approx_char_overstep.ceil() as usize;

						// Checking that the cut text amount is not too large for this span
						assert!(char_overstep <= orig_span_len);

						span = &span[0..orig_span_len - char_overstep];
						(span_as_string, subsurface_width, next_total_width) = compute_span_data(span)?;

						// Double-checking that the monospace property holds
						assert!(subsurface_width == first_char_pixel_width * span.len() as u32);

						did_monospace_cutting = true;
					}
					else {
						log::error!("The monospace property did not hold! Finding the offending character(s).");

						for (i, c) in span.iter().enumerate() {
							let char_width = chosen_font.size_of_char(*c)?.0;

							if char_width != first_char_pixel_width {
								log::error!("Character #{i} ({c}) had a width of {char_width} instead of {first_char_pixel_width}.");
							}
						}
					}
				}

				if !did_monospace_cutting {
					log::debug!("Doing manual text span cutting (quite inefficient)");

					// TODO: perhaps use `get_reference_time`?
					let time_before = std::time::Instant::now();

					/* TODO: could I use the monospace cutting as a lossy estimator,
					and use that as a faster starting point for this cutting? */
					while next_total_width > max_texture_width {
						span = &span[0..span.len() - 1];
						(span_as_string, subsurface_width, next_total_width) = compute_span_data(span)?;
					}

					log::debug!("That took this many milliseconds: {}", time_before.elapsed().as_millis());
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
		).to_generic_result()?;

		let mut dest_rect = Rect::new(0, 0, 1, 1);

		for mut subsurface in subsurfaces {
			subsurface.set_blend_mode(BlendMode::None).to_generic_result()?;

			(dest_rect.w, dest_rect.h) = (subsurface.width() as i32, subsurface.height() as i32);
			subsurface.blit(None, &mut joined_surface, dest_rect).to_generic_result()?;
			dest_rect.x += dest_rect.w;
		}

		Ok(joined_surface)
	}

	fn make_text_surface(&mut self, font_info: &text::FontInfo,
		text_display_info: &text::TextDisplayInfo) -> GenericResult<Surface<'a>> {

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
				let mut corrected = Surface::new(max_width, needed_height, blank_surface.pixel_format_enum()).to_generic_result()?;
				blank_surface.set_blend_mode(BlendMode::None).to_generic_result()?;
				blank_surface.blit(None, &mut corrected, None).to_generic_result()?;
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
		/*
		TODO: introduce an optimization for texture loading that works like this:
		1. A new `TextureCreationInfo` variant called `PreloadedSurface`, that contains the info for a surface loaded on another task (probably via `spawn_blocking`)
		2. Load surfaces in on other tasks (provide bytes via `RWops`), and then, since surfaces aren't `Send`, serialize them to some other type (which would be contained within `PreloadedSurface`)
		3. Send those `PreloadedSurface`s over to the main thread when it's time to load, and directly write the pixel data into an allocated texture (or, make a surface, and then convert that to a texture); regardless, ensure that a fast pixel format is used

		This would be useful, since texture loading is still pretty slow, partly because compressed formats like `.png` have to
		be decoded, and have their formats/pixel orders converted in various ways. This is the majority of the performance impact
		when loading in textures.

		With this, probably add some code into this function that emits a warning whenever texture creation takes too long here (e.g. over 10ms).
		*/

		match creation_info {
			// Use this whenever possible (whenever you can preload data into byte form)!
			TextureCreationInfo::RawBytes(bytes) =>
				self.texture_creator.load_texture_bytes(bytes),

			TextureCreationInfo::Path(path) =>
				self.texture_creator.load_texture(path as &str),

			TextureCreationInfo::Url(url) => {
				use futures::executor::block_on;

				let response = block_on(request::get(url, None))?;
				let bytes = block_on(response.bytes())?;

				self.texture_creator.load_texture_bytes(&bytes)
			}

			TextureCreationInfo::Text((font_info, text_display_info)) => {
				let surface = self.make_text_surface(font_info, text_display_info)?;

				assert!(surface.width() >= text_display_info.pixel_area.0);
				assert!(surface.height() == text_display_info.pixel_area.1);

				Ok(self.texture_creator.create_texture_from_surface(surface)?)
			}
		}.to_generic_result()
	}
}
