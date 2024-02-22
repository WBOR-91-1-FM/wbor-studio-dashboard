use crate::{
	texture::{TextDisplayInfo, TextureCreationInfo},

	spinitron::model::{SpinitronModelName, NUM_SPINITRON_MODEL_TYPES},

	utility_types::{
		vec2f::Vec2f,
		update_rate::UpdateRate,
		generic_result::GenericResult,
		dynamic_optional::DynamicOptional
	},

	window_tree::{
		ColorSDL,
		Window,
		WindowContents,
		WindowUpdaterParams,
		PossibleWindowUpdater
	},

	window_tree_defs::shared_window_state::SharedWindowState
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

	fn spinitron_model_window_updater_fn((window, texture_pool,
		shared_state, area_drawn_to_screen): WindowUpdaterParams) -> GenericResult<()> {

		let inner_shared_state: &SharedWindowState = shared_state.get_inner_value();
		let spinitron_state = &inner_shared_state.spinitron_state;

		let individual_window_state: &SpinitronModelWindowState = window.get_state();
		let model_name = individual_window_state.model_name;

		let model = spinitron_state.get_model_by_name(model_name);

		let model_as_string = format!("{} ", model.to_string());

		let texture_creation_info = if let Some(text_color) = individual_window_state.maybe_text_color {
			TextureCreationInfo::Text((
				&inner_shared_state.font_info,

				TextDisplayInfo {
					text: &model_as_string,
					color: text_color,

					/* TODO:
					- Pass in the scroll fn
					- Figure out how to make a scroll fn that pauses a bit at each end
					*/

					scroll_fn: |secs_since_unix_epoch| {
						(secs_since_unix_epoch.sin() * 0.5 + 0.5, false)
					},

					// TODO: why does cutting the max pixel width in half still work?
					max_pixel_width: area_drawn_to_screen.width(),
					pixel_height: area_drawn_to_screen.height()
				}
			))
		}
		else {
			match model.get_texture_creation_info() {
				Some(texture_creation_info) => texture_creation_info,
				None => inner_shared_state.fallback_texture_creation_info.clone()
			}
		};

		// TODO: see if threading will be needed for updating textures as well
		window.update_texture_contents(
			spinitron_state.model_was_updated(model_name),
			texture_pool,
			&texture_creation_info,
			&inner_shared_state.fallback_texture_creation_info
		)
	}

	////////// Making the model windows

	let spinitron_model_window_updater: PossibleWindowUpdater = Some((spinitron_model_window_updater_fn, model_update_rate));

	all_model_windows_info.iter().flat_map(|info| {
		let mut output_windows = vec![];

		if let Some(texture_window_info) = &info.texture_window {
			let texture_window = Window::new(
				spinitron_model_window_updater,

				DynamicOptional::new(SpinitronModelWindowState {
					model_name: info.model_name, maybe_text_color: None
				}),

				WindowContents::Nothing,
				texture_window_info.border_color,
				texture_window_info.tl,
				texture_window_info.size,
				None
			);

			output_windows.push(texture_window);
		}

		if let Some(text_window_info) = &info.text_window {
			let text_window = Window::new(
				spinitron_model_window_updater,

				DynamicOptional::new(SpinitronModelWindowState {
					model_name: info.model_name, maybe_text_color: Some(info.text_color)
				}),

				WindowContents::Nothing,
				text_window_info.border_color,
				text_window_info.tl,
				text_window_info.size,
				None
			);

			output_windows.push(text_window)
		}

		output_windows
	}).collect()
}
