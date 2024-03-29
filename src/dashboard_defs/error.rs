use std::borrow::Cow;

use crate::{
	texture::{TextDisplayInfo, TextureCreationInfo},

	utility_types::{
		vec2f::Vec2f,
		update_rate::UpdateRate,
		generic_result::MaybeError,
		dynamic_optional::DynamicOptional
	},

	window_tree::{
		Window,
		ColorSDL,
		WindowContents,
		WindowUpdaterParams
	},

	dashboard_defs::shared_window_state::SharedWindowState
};

// TODO: maybe replace this with the SDL message box?
pub fn make_error_window(top_left: Vec2f, size: Vec2f, update_rate: UpdateRate,
	background_contents: WindowContents, text_color: ColorSDL) -> Window {

	struct ErrorWindowState {
		prev_error: Option<String>,
		text_color: ColorSDL
	}

	pub fn error_updater_fn(params: WindowUpdaterParams) -> MaybeError {
		let inner_shared_state = params.shared_window_state.get::<SharedWindowState>();
		let individual_state = params.window.get_state::<ErrorWindowState>();

		let (curr_error, cached_error) = (
			&inner_shared_state.curr_dashboard_error, &individual_state.prev_error
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

						max_pixel_width: params.area_drawn_to_screen.width(),
						pixel_height: params.area_drawn_to_screen.height()
					}
				));

				let WindowContents::Many(all_contents) = params.window.get_contents_mut()
				else {panic!("The error window contents was expected to be a list!")};
				let text_texture = &mut all_contents[1];

				text_texture.update_as_texture(
					true,
					params.texture_pool,
					&texture_creation_info,
					&inner_shared_state.fallback_texture_creation_info
				)?;

				params.window.set_draw_skipping(false);
			}
			else {
				params.window.set_draw_skipping(true);
			}

			params.window.get_state_mut::<ErrorWindowState>().prev_error = curr_error.clone();
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
