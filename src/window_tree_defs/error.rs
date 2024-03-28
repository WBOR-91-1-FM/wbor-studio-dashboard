use std::borrow::Cow;

use crate::{
	texture::{TextDisplayInfo, TextureCreationInfo},

	utility_types::{
		vec2f::Vec2f,
		update_rate::UpdateRate,
		generic_result::GenericResult,
		dynamic_optional::DynamicOptional
	},

	window_tree::{
		Window,
		ColorSDL,
		WindowContents,
		WindowUpdaterParams
	},

	window_tree_defs::shared_window_state::SharedWindowState
};

// TODO: maybe replace this with the SDL message box?
pub fn make_error_window(top_left: Vec2f, size: Vec2f, update_rate: UpdateRate,
	background_contents: WindowContents, text_color: ColorSDL) -> Window {

	struct ErrorWindowState {
		prev_error: Option<String>,
		text_color: ColorSDL
	}

	pub fn error_updater_fn((window, texture_pool, shared_state, area_drawn_to_screen): WindowUpdaterParams) -> GenericResult<()> {
		let inner_shared_state = shared_state.get_inner_value::<SharedWindowState>();
		let individual_state = window.get_state::<ErrorWindowState>();

		let (curr_error, cached_error) = (
			&inner_shared_state.dashboard_error, &individual_state.prev_error
		);

		// This means that the error changed (or disappeared)!
		if curr_error != cached_error {
			if let Some(ref inner_curr_error) = curr_error {
				let texture_creation_info = TextureCreationInfo::Text((
					inner_shared_state.font_info,

					TextDisplayInfo {
						text: Cow::Borrowed(inner_curr_error),
						color: individual_state.text_color,

						scroll_fn: |seed, _| {
							let repeat_rate_secs = 2.0;
							((seed % repeat_rate_secs) / repeat_rate_secs, true)
						},

						max_pixel_width: area_drawn_to_screen.width(),
						pixel_height: area_drawn_to_screen.height()
					}
				));

				let WindowContents::Many(all_contents) = window.get_contents_mut()
				else {panic!("The error window contents was expected to be a list!")};
				let text_texture = &mut all_contents[1];

				text_texture.update_as_texture(
					true,
					texture_pool,
					&texture_creation_info,
					&inner_shared_state.fallback_texture_creation_info
				)?;

				window.set_draw_skipping(false);
			}
			else {
				window.set_draw_skipping(true);
			}

			window.get_state_mut::<ErrorWindowState>().prev_error = curr_error.clone();
		}

		Ok(())
	}

	let mut error_window = Window::new(
		Some((error_updater_fn, update_rate)),
		DynamicOptional::new(ErrorWindowState {prev_error: None, text_color}),
		WindowContents::Many(vec![background_contents, WindowContents::Nothing]),
		None,
		top_left,
		size,
		None
	);

	error_window.set_draw_skipping(true);

	error_window
}
