use std::borrow::Cow;

use crate::{
	dashboard_defs::{
		easing_fns,
		funky_remake_transitions,
		shared_window_state::SharedWindowState
	},

	texture::{
		pool::TextureCreationInfo,
		text::{DisplayText, TextDisplayInfo}
	},

	utility_types::{
		time::*,
		vec2f::Vec2f,
		generic_result::*,
		update_rate::UpdateRate,
		dynamic_optional::DynamicOptional
	},

	window_tree::{
		Window,
		ColorSDL,
		WindowContents,
		WindowUpdaters,
		WindowUpdaterParams
	},

	spinitron::model::{SpinitronModelName, NUM_SPINITRON_MODEL_TYPES}
};

struct SpinitronModelWindowState {
	model_name: SpinitronModelName,
	maybe_text_color: Option<ColorSDL> // If this is `None`, it is not a text window
}

pub struct SpinitronModelWindowInfo {
	pub tl: Vec2f,
	pub size: Vec2f,
	pub border_color: Option<ColorSDL>
}

pub struct SpinitronModelWindowsInfo {
	pub model_name: SpinitronModelName,
	pub texture_window: Option<SpinitronModelWindowInfo>,
	pub text_window: Option<SpinitronModelWindowInfo>,
	pub text_color: ColorSDL
}

//////////

pub fn make_spinitron_windows(
	all_model_windows_info: &[SpinitronModelWindowsInfo; NUM_SPINITRON_MODEL_TYPES],
	model_update_rate: UpdateRate) -> Vec<Window> {

	/* Note: the drawn size passed into this does not account for aspect ratio correction.
	For Spinitron models, the size is only needed for spin textures all and text textures.
	For spin textures, the you can figure out the corrected texture size since spin textures
	are square, and you can get the corrected size by taking the min of the two components
	of `area_drawn_to_screen`. */
	fn spinitron_model_window_updater_fn(params: WindowUpdaterParams) -> MaybeError {
		let inner_shared_state = params.shared_window_state.get_mut::<SharedWindowState>();
		let individual_window_state = params.window.get_state::<SpinitronModelWindowState>();
		let model_name = individual_window_state.model_name;
		let window_size_pixels = params.area_drawn_to_screen;

		////////// The spin texture window is the window designated for updating the Spinitron state

		let is_text_window = individual_window_state.maybe_text_color.is_some();

		if model_name == SpinitronModelName::Spin && !is_text_window {
			let spin_texture_window_size = window_size_pixels.0.min(window_size_pixels.1);
			let size_2d = (spin_texture_window_size, spin_texture_window_size);
			inner_shared_state.spinitron_state.update(size_2d, &mut inner_shared_state.error_state);
		}

		//////////

		let spinitron_state = &mut inner_shared_state.spinitron_state;

		let should_update_texture =
			spinitron_state.model_was_updated(model_name) ||
			matches!(params.window.get_contents(), WindowContents::Nothing);

		if !should_update_texture {return Ok(());}

		//////////

		let texture_creation_info = if is_text_window {
			let model_text = spinitron_state.model_to_string(model_name);

			TextureCreationInfo::Text((
				Cow::Borrowed(inner_shared_state.font_info),

				TextDisplayInfo {
					text: DisplayText::new(&model_text),
					color: individual_window_state.maybe_text_color.unwrap(),
					pixel_area: window_size_pixels, // TODO: why does cutting the max pixel width in half still work?

					/* TODO:
					- Pass this in
					- Why doesn't this scroll when the text is short enough? Good, but not programmed in...
					*/
					scroll_easer: easing_fns::scroll::OSCILLATE_NO_WRAP,
					scroll_speed_multiplier: 2.0
				}
			))
		}
		else {
			spinitron_state.get_cached_texture_creation_info(model_name)
		};

		//////////

		let default_duration = Duration::seconds(3);

		let intermediate_info = funky_remake_transitions::IntermediateTextureTransitionInfo {
			percent_chance_to_show_rand_intermediate_texture: 0.2,
			rand_duration_range_for_intermediate: (0.4, 2.2),
			max_random_transitions: 10
		};

		//////////

		funky_remake_transitions::update_as_texture_with_funky_remake_transition(
			params.window.get_contents_mut(),
			params.texture_pool,
			&texture_creation_info,
			default_duration,
			&mut inner_shared_state.rand_generator,
			inner_shared_state.get_fallback_texture_creation_info,
			intermediate_info
		)
	}

	////////// Making the model windows

	let spinitron_model_window_updaters: WindowUpdaters = vec![(spinitron_model_window_updater_fn, model_update_rate)];

	// TODO: perhaps for making multiple model windows, allow for an option to have sub-model-windows
	all_model_windows_info.iter().flat_map(|general_info| {
		let mut output_windows = Vec::new();

		let mut maybe_make_model_window =
			|maybe_info: &Option<SpinitronModelWindowInfo>, maybe_text_color: Option<ColorSDL>| {

			if let Some(info) = maybe_info {
				output_windows.push(Window::new(
					spinitron_model_window_updaters.clone(),

					DynamicOptional::new(SpinitronModelWindowState {
						model_name: general_info.model_name,
						maybe_text_color
					}),

					WindowContents::Nothing,
					info.border_color,
					info.tl,
					info.size,
					vec![]
				));
			}
		};

		maybe_make_model_window(&general_info.texture_window, None);
		maybe_make_model_window(&general_info.text_window, Some(general_info.text_color));

		output_windows
	}).collect()
}
