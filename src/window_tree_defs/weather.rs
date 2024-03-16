/* TODO:
- Actually implement this
- Make the general structure of the text updater fns less repetitive
*/

use std::borrow::Cow;

use crate::{
	texture::{TextDisplayInfo, TextureCreationInfo},

	utility_types::{
        vec2f::Vec2f,
        update_rate::UpdateRateCreator,
        generic_result::GenericResult,
		dynamic_optional::DynamicOptional
	},

	window_tree::{
		ColorSDL,
        Window,
        WindowContents,
        WindowUpdaterParams
	},

	window_tree_defs::shared_window_state::SharedWindowState
};

// TODO: fill this with stuff
struct WeatherWindowState {

}

pub fn weather_updater_fn((window, texture_pool, shared_state, area_drawn_to_screen): WindowUpdaterParams) -> GenericResult<()> {
	let weather_changed = true;
	let weather_string = "Rain (32f). So cold. ";
	let weather_text_color = ColorSDL::BLACK;

	// let individual_window_state = window.get_state::<WeatherWindowState>();
	let inner_shared_state = shared_state.get_inner_value::<SharedWindowState>();

	let texture_creation_info = TextureCreationInfo::Text((
		inner_shared_state.font_info,

		TextDisplayInfo {
			text: Cow::Borrowed(weather_string),
			color: weather_text_color,

			scroll_fn: |seed, _| {
				let repeat_rate_secs = 3.0;
				let base_scroll = (seed % repeat_rate_secs) / repeat_rate_secs;
				(1.0 - base_scroll, true)
			},

			max_pixel_width: area_drawn_to_screen.width(),
			pixel_height: area_drawn_to_screen.height()
		}
	));

	window.update_texture_contents(
		weather_changed,
		texture_pool,
		&texture_creation_info,
		&inner_shared_state.fallback_texture_creation_info
	)?;

	Ok(())
}

// TODO: pass more params in here
pub fn make_weather_window(update_rate_creator: &UpdateRateCreator) -> Window {
	let weather_update_rate = update_rate_creator.new_instance(60.0);

	Window::new(
		Some((weather_updater_fn, weather_update_rate)),
		DynamicOptional::new(WeatherWindowState {}),
		WindowContents::Color(ColorSDL::RGB(255, 0, 255)),
		Some(ColorSDL::RED),
		Vec2f::ZERO,
		Vec2f::new(0.2, 0.2),
		None
	)
}
