use crate::{
	utility_types::{
		update_rate::UpdateRate,
		dynamic_optional::DynamicOptional,
		generic_result::GenericResult, vec2f::Vec2f
	},

	spinitron::model::SpinitronModelName,
	texture::{TextDisplayInfo, TextureCreationInfo},

	window_tree::{
		ColorSDL,
		Window, WindowContents,
		WindowUpdaterParams, PossibleWindowUpdater
	},

	window_tree_defs::shared_window_state::SharedWindowState
};

struct SpinitronModelWindowState {
	model_name: SpinitronModelName,
	is_text_window: bool
}

pub fn make_spinitron_windows(
	model_window_size: Vec2f, gap_size: f32,
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

		let text_color = ColorSDL::RGBA(255, 0, 0, 178);
		let text_to_display = format!("{} ", model.to_string());

		// TODO: vary the params based on the text window
		let texture_creation_info = if individual_window_state.is_text_window {
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

	// `tl` = top left

	let spin_tl = Vec2f::new_from_one(gap_size);
	let playlist_tl = spin_tl.translate_x(model_window_size.x() + gap_size);

	let persona_tl = spin_tl.translate_y(model_window_size.y() + gap_size);
	let show_tl = Vec2f::new(playlist_tl.x(), persona_tl.y());

	let (text_tl, text_size) = (Vec2f::ZERO, Vec2f::new(1.0, 0.1));

	let spinitron_model_window_metadata = [
		(SpinitronModelName::Spin, spin_tl),
		(SpinitronModelName::Playlist, playlist_tl),
		(SpinitronModelName::Persona, persona_tl),
		(SpinitronModelName::Show, show_tl)
	];

	spinitron_model_window_metadata.iter().map(|metadata| {
		let model_name = metadata.0;

		let text_child = Window::new(
			spinitron_model_window_updater,

			DynamicOptional::new(SpinitronModelWindowState {
				model_name, is_text_window: true
			}),

			WindowContents::Nothing,
			Some(ColorSDL::GREEN),
			text_tl,
			text_size,
			None
		);

		Window::new(
			spinitron_model_window_updater,

			DynamicOptional::new(SpinitronModelWindowState {
				model_name, is_text_window: false
			}),

			WindowContents::Nothing,
			Some(ColorSDL::BLUE),
			metadata.1,
			model_window_size,
			Some(vec![text_child])
		)
	}).collect()
}
