use std::borrow::Cow;
use sdl2::ttf::FontStyle;

use crate::{
	texture::{
		DisplayText,
		TextDisplayInfo,
		TextureCreationInfo
	},

	window_tree::{
		Window,
		ColorSDL,
		WindowContents,
		WindowUpdaterParams
	},

	utility_types::{
		vec2f::Vec2f,
		update_rate::UpdateRate,
		generic_result::MaybeError,
		dynamic_optional::DynamicOptional
	},

	dashboard_defs::shared_window_state::SharedWindowState
};

pub fn make_credit_window(top_left: Vec2f, size: Vec2f, text_color: ColorSDL, text: &'static str) -> Window {
	struct CreditWindowState {
		text: &'static str,
		text_color: ColorSDL
	}

	// TODO: genericize this whole updater-fn scheme (it's very repeated in a lot of places)
	fn credit_updater_fn(params: WindowUpdaterParams) -> MaybeError {
		if let WindowContents::Texture(_) = params.window.get_contents() {return Ok(());}

		let individual_window_state = params.window.get_state::<CreditWindowState>();
		let inner_shared_state = params.shared_window_state.get::<SharedWindowState>();

		let mut italicized_font_info = inner_shared_state.font_info.clone();
		italicized_font_info.style = FontStyle::ITALIC;

		let texture_creation_info = TextureCreationInfo::Text((
			&italicized_font_info,

			TextDisplayInfo {
				text: DisplayText::new(Cow::Borrowed(individual_window_state.text)),
				color: individual_window_state.text_color,
				pixel_area: params.area_drawn_to_screen,
				scroll_fn: |seed, _| (seed.sin() * 0.5 + 0.5, false)
			}
		));

		params.window.get_contents_mut().update_as_texture(
			true,
			params.texture_pool,
			&texture_creation_info,
			&inner_shared_state.fallback_texture_creation_info
		)
	}

	Window::new(
		Some((credit_updater_fn, UpdateRate::ALMOST_NEVER)),
		DynamicOptional::new(CreditWindowState {text, text_color}),
		WindowContents::Color(ColorSDL::RGB(128, 0, 32)),
		Some(ColorSDL::RED),
		top_left,
		size,
		None
	)
}
