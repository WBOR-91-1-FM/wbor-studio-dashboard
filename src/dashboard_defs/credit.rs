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
		easing_fns,
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
			let italicized_font_info = inner_shared_state.font_info.with_style(sdl2::ttf::FontStyle::ITALIC);
			(Cow::Owned(italicized_font_info), "")
		}

		fn extract_text(&self, _: &SharedWindowState) -> Cow<str> {
			Cow::Borrowed(self)
		}

		fn extract_texture_contents(window_contents: &mut WindowContents) -> &mut WindowContents {
			window_contents
		}
	}

	let fields = updatable_text_pattern::UpdatableTextWindowFields {
		inner: text,
		text_color,
		scroll_easer: easing_fns::scroll::OSCILLATE_NO_WRAP,
		scroll_speed_multiplier: 1.75,
		update_rate: UpdateRate::ALMOST_NEVER,
		maybe_border_color: Some(border_color)
	};

	updatable_text_pattern::make_window(fields, top_left, size, WindowContents::Nothing)
}
