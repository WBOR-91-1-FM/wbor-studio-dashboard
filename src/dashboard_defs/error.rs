use std::borrow::Cow;

use crate::{
	utility_types::{
		vec2f::Vec2f,
		update_rate::UpdateRate
	},

	window_tree::{
		Window,
		ColorSDL,
		WindowContents,
		WindowUpdaterParams
	},

	dashboard_defs::{
		updatable_text_pattern,
		shared_window_state::SharedWindowState
	}
};

// TODO: maybe replace this with the SDL message box?
pub fn make_error_window(top_left: Vec2f, size: Vec2f, update_rate: UpdateRate,
	background_contents: WindowContents, text_color: ColorSDL) -> Window {

	type ErrorWindowState = Option<String>; // This is the previous error

	impl updatable_text_pattern::UpdatableTextWindowMethods for ErrorWindowState {
		fn should_skip_update(updater_params: &mut WindowUpdaterParams) -> bool {
			let inner_shared_state = updater_params.shared_window_state.get::<SharedWindowState>();

			let wrapped_individual_state = updater_params.window.get_state_mut
				::<updatable_text_pattern::UpdatableTextWindowFields<ErrorWindowState>>();

			let prev_error = &wrapped_individual_state.inner;

			let (curr_error, cached_error) = (
				&inner_shared_state.curr_dashboard_error, prev_error
			);

			// This means that the error changed (or disappeared)!
			if curr_error != cached_error {
				let skip_update = curr_error.is_none();
				wrapped_individual_state.inner = curr_error.clone();
				updater_params.window.set_draw_skipping(skip_update);
				skip_update
			}
			else {
				true
			}
		}

		fn compute_within_updater<'a>(inner_shared_state: &'a SharedWindowState) -> updatable_text_pattern::ComputedInTextUpdater<'a> {
			(Cow::Borrowed(inner_shared_state.font_info), " ")
		}

		fn extract_text(&self) -> Cow<str> {
			Cow::Borrowed(self.as_ref().unwrap())
		}

		fn extract_texture_contents(window_contents: &mut WindowContents) -> &mut WindowContents {
			let WindowContents::Many(all_contents) = window_contents
			else {panic!("The error window contents was expected to be a list!")};
			&mut all_contents[1]
		}
	}

	let fields = updatable_text_pattern::UpdatableTextWindowFields {
		inner: None,
		text_color,

		scroll_fn: |seed, _| {
			let repeat_rate_secs = 2.0;
			((seed % repeat_rate_secs) / repeat_rate_secs, true)
		},

		update_rate,
		maybe_border_color: None
	};

	let mut window = updatable_text_pattern::make_window(
		fields, top_left, size,
		WindowContents::Many(vec![background_contents, WindowContents::Nothing])
	);

	window.set_draw_skipping(true);
	window

}
