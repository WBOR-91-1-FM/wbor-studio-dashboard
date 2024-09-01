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

pub fn make_credit_window(top_left: Vec2f, size: Vec2f,
	border_color: ColorSDL, text_color: ColorSDL, text: String) -> Window {

	type CreditWindowState = String;

	impl updatable_text_pattern::UpdatableTextWindowMethods for CreditWindowState {
		fn should_skip_update(updater_params: &mut WindowUpdaterParams) -> bool {
			let window_contents = updater_params.window.get_contents();
			matches!(window_contents, WindowContents::Texture(_))
		}

		fn compute_within_updater<'a>(inner_shared_state: &'a SharedWindowState) -> updatable_text_pattern::ComputedInTextUpdater<'a> {
			let mut italicized_font_info = inner_shared_state.font_info.clone();
			italicized_font_info.style = sdl2::ttf::FontStyle::ITALIC;
			(Cow::Owned(italicized_font_info), "")
		}

		fn extract_text(&self) -> Cow<str> {
			Cow::Borrowed(self)
		}

		fn extract_texture_contents(window_contents: &mut WindowContents) -> &mut WindowContents {
			window_contents
		}
	}

	let fields = updatable_text_pattern::UpdatableTextWindowFields {
		inner: text,
		text_color,
		scroll_fn: |seed, _| ((seed * 5.0).sin() * 0.5 + 0.5, false),
		update_rate: UpdateRate::ALMOST_NEVER,
		maybe_border_color: Some(border_color)
	};

	updatable_text_pattern::make_window(fields, top_left, size, WindowContents::Nothing)
}
