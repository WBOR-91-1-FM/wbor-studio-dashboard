use std::borrow::Cow;

use crate::{
	dashboard_defs::shared_window_state::SharedWindowState,

	spinitron::model::{Spin, SpinitronModelName, NUM_SPINITRON_MODEL_TYPES},

	texture::{
		DisplayText,
		TextDisplayInfo,
		TextureCreationInfo
	},

	utility_types::{
		vec2f::Vec2f,
		generic_result::*,
		update_rate::UpdateRate,
		dynamic_optional::DynamicOptional
	},

	window_tree::{
		Window,
		ColorSDL,
		WindowContents,
		WindowUpdaterParams,
		PossibleWindowUpdater
	}
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
		let spinitron_state = &mut inner_shared_state.spinitron_state;

		let individual_window_state = params.window.get_state::<SpinitronModelWindowState>();
		let model_name = individual_window_state.model_name;
		let window_size_pixels = params.area_drawn_to_screen;

		//////////

		let should_update_texture =
			spinitron_state.model_was_updated(model_name) ||
			matches!(params.window.get_contents(), WindowContents::Nothing);

		if !should_update_texture {return Ok(());}

		//////////

		let texture_creation_info = if let Some(text_color) = individual_window_state.maybe_text_color {
			let text = if spinitron_state.is_spin_and_just_expired(model_name) {
				Cow::Borrowed(Spin::to_string_when_spin_is_expired())
			}
			else {
				Cow::Owned(spinitron_state.get_model_by_name(model_name).to_string())
			};

			TextureCreationInfo::Text((
				Cow::Borrowed(inner_shared_state.font_info),

				TextDisplayInfo {
					text: DisplayText::new(&text),
					color: text_color,
					pixel_area: window_size_pixels, // TODO: why does cutting the max pixel width in half still work?

					/* TODO:
					- Pass this in
					- Make a scroll fn util file
					- Why doesn't this scroll when the text is short enough? Good, but not programmed in...
					*/
					scroll_fn: |seed, _| (seed.sin() * 0.5 + 0.5, false)

				}
			))
		}
		else {
			// Registering the aspect-ratio-corrected spin window size
			if matches!(model_name, SpinitronModelName::Spin) {
				let size = window_size_pixels.0.min(window_size_pixels.1);
				spinitron_state.register_spin_window_size((size, size));
			}

			spinitron_state.get_cached_texture_creation_info(model_name)
		};

		params.window.get_contents_mut().update_as_texture(
			true,
			params.texture_pool,
			&texture_creation_info,
			inner_shared_state.fallback_texture_creation_info
		)
	}

	////////// Making the model windows

	let spinitron_model_window_updater: PossibleWindowUpdater = Some((spinitron_model_window_updater_fn, model_update_rate));

	// TODO: perhaps for making multiple model windows, allow for an option to have sub-model-windows
	all_model_windows_info.iter().flat_map(|general_info| {
		let mut output_windows = Vec::new();

		let mut maybe_make_model_window =
			|maybe_info: &Option<SpinitronModelWindowInfo>, maybe_text_color: Option<ColorSDL>| {

			if let Some(info) = maybe_info {
				output_windows.push(Window::new(
					spinitron_model_window_updater,

					DynamicOptional::new(SpinitronModelWindowState {
						model_name: general_info.model_name,
						maybe_text_color
					}),

					WindowContents::Nothing,
					info.border_color,
					info.tl,
					info.size,
					None
				));
			}
		};

		maybe_make_model_window(&general_info.texture_window, None);
		maybe_make_model_window(&general_info.text_window, Some(general_info.text_color));

		output_windows
	}).collect()
}
