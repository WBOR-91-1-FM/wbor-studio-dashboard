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
	pub texture_window: SpinitronModelWindowInfo,
	pub text_window: SpinitronModelWindowInfo,
	pub text_color: ColorSDL
}

//////////

pub fn make_spinitron_windows(
	all_model_windows_info: &[SpinitronModelWindowsInfo; NUM_SPINITRON_MODEL_TYPES],
	model_update_rate: UpdateRate) -> Vec<Window> {

	/* TODO: add the ability to have multiple updaters per window
	(with different update rates). Or, do async requests. */
	fn spinitron_model_window_updater_fn((window, texture_pool,
		shared_state, area_drawn_to_screen): WindowUpdaterParams) -> GenericResult<()> {

		let inner_shared_state: &SharedWindowState = shared_state.get_inner_value();
		let spinitron_state = &inner_shared_state.spinitron_state;

		let individual_window_state: &SpinitronModelWindowState = window.get_state();
		let model_name = individual_window_state.model_name;

		let model = spinitron_state.get_model_by_name(model_name);
		let model_was_updated = spinitron_state.model_was_updated(model_name);

		let text_to_display = format!("{} ", model.to_string());

		let texture_creation_info = if let Some(text_color) = individual_window_state.maybe_text_color {
			TextureCreationInfo::Text((
				&inner_shared_state.font_info,

				TextDisplayInfo {
					text: text_to_display,
					color: text_color,

					// TODO: pass in the scroll fn too
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
			model_was_updated,
			texture_pool,
			&texture_creation_info,
			&inner_shared_state.fallback_texture_creation_info
		)
	}

	////////// Making the model windows

	let spinitron_model_window_updater: PossibleWindowUpdater = Some((spinitron_model_window_updater_fn, model_update_rate));

	// TODO: stop the repetition here

	let mut spinitron_windows: Vec<Window> = all_model_windows_info.iter().map(|info| {
		Window::new(
			spinitron_model_window_updater,

			DynamicOptional::new(SpinitronModelWindowState {
				model_name: info.model_name, maybe_text_color: None
			}),

			WindowContents::Nothing,
			info.texture_window.border_color,
			info.texture_window.tl,
			info.texture_window.size,
			None
		)
	}).collect();

	spinitron_windows.extend(all_model_windows_info.iter().map(|info| {
		Window::new(
			spinitron_model_window_updater,

			DynamicOptional::new(SpinitronModelWindowState {
				model_name: info.model_name, maybe_text_color: Some(info.text_color)
			}),

			WindowContents::Nothing,
			info.text_window.border_color,
			info.text_window.tl,
			info.text_window.size,
			None
		)
	}).collect::<Vec<Window>>());

	spinitron_windows
}
