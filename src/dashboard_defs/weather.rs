use std::{collections::HashMap, borrow::Cow};

use crate::{
	request,

	texture::{
		pool::TextureCreationInfo,
		text::{DisplayText, TextDisplayInfo}
	},

	utility_types::{
		vec2f::Vec2f,
		generic_result::*,
		dynamic_optional::DynamicOptional,
		update_rate::{UpdateRateCreator, Seconds},
		continually_updated::{ContinuallyUpdated, Updatable}
	},

	window_tree::{
		ColorSDL,
		Window,
		WindowContents,
		WindowUpdaterParams
	},

	dashboard_defs::{
		easing_fns,
		shared_window_state::SharedWindowState
	}
};

lazy_static::lazy_static!(
	// Based on the weather codes from here: https://docs.tomorrow.io/reference/weather-data-layers
	static ref WEATHER_CODE_MAPPING: HashMap<u16, (&'static str, &'static str)> = HashMap::from([
		(0, ("unknown", "❓")),
		(1000, ("clear", "☀️")),
		(1001, ("cloudy", "☁️")),
		(1100, ("mostly clear", "🌤️")),
		(1101, ("partly cloudy", "⛅")),
		(1102, ("mostly cloudy", "🌥️")),

		(2000, ("foggy", "🌫️🌫️")),
		(2100, ("just a little bit of fog", "🌫️")),

		(3000, ("a little bit of wind", "🍃")),
		(3001, ("some wind", "💨")),
		(3002, ("quite windy", "🌬️")),

		(4000, ("a bit of a drizzle", "🌦️")),
		(4001, ("rainy", "🌧️")),
		(4200, ("just a little bit of rain", "🌦️🌦️")),
		(4201, ("very very rainy", "🌧️🌧️")),

		(5000, ("snowy", "❄️")),
		(5001, ("some flurries", "🌨️")),
		(5100, ("a little bit of snow", "🌨️❄️")),
		(5101, ("quite a lot of snow", "🌨️️❄❄️")),

		(6000, ("freezing drizzle", "🌧️❄️")),
		(6001, ("freezing rain", "🌧️🌧️🌧️❄")),
		(6200, ("light freezing rain", "🌧️🌧️❄️")),
		(6201, ("heavy freezing rain", "🌧️🌧️🌧️🌧️❄️")),

		(7000, ("ice pellets - watch your head", "🧊🧊")),
		(7101, ("heavy ice pellets - dangerous!", "🧊🧊🧊")),
		(7102, ("light ice pellets - you'll be okay", "🧊")),
		(8000, ("thunderstorm - beware!", "⛈️"))
	]);
);

//////////

#[derive(Clone)]
struct WeatherStateData {
	text_color: ColorSDL,

	request_url: String,
	weather_changed: bool,

	// Rounded temperature, weather code descriptor, and associated emoji
	curr_weather_info: Option<(i16, &'static str, &'static str)>
}

impl Updatable for WeatherStateData {
	type Param = ();

	// TODO: while the weather is fetched every 10 minutes, use the other 9 minutes' forecasts for more updated data
	async fn update(&mut self, _: &Self::Param) -> MaybeError {
		// return Ok(()); // Use this line when developing locally, and you don't want to rate-limit this API in the studio!

		let all_info_json: serde_json::Value = request::as_type(request::get(&self.request_url)).await?;

		let current_weather_json = &all_info_json["data"]["timelines"][0]["intervals"][0]["values"];

		let associated_code = current_weather_json["weatherCode"].as_i64().unwrap() as u16;
		let (weather_code_descriptor, associated_emoji) = WEATHER_CODE_MAPPING.get(&(associated_code)).unwrap();

		let rounded_temperature = current_weather_json["temperature"].as_f64().unwrap().round() as i16;

		let new_info = Some((rounded_temperature, weather_code_descriptor as &str, associated_emoji as &str));
		self.weather_changed = new_info != self.curr_weather_info;

		if self.weather_changed {
			self.curr_weather_info = new_info;
		}

		Ok(())
	}
}

//////////

// TODO: use the updatable text pattern here
fn weather_updater_fn(params: WindowUpdaterParams) -> MaybeError {
	let inner_shared_state = params.shared_window_state.get_mut::<SharedWindowState>();
	let individual_window_state = params.window.get_state_mut::<ContinuallyUpdated<WeatherStateData>>();

	individual_window_state.update(&(), &mut inner_shared_state.error_state)?;
	let inner = individual_window_state.get_data();

	if !inner.weather_changed || inner.curr_weather_info.is_none() {
		return Ok(());
	}

	let (rounded_temperature, weather_code_descriptor, associated_emoji) = inner.curr_weather_info.unwrap();
	let weather_string = format!("Weather: {rounded_temperature}° and {weather_code_descriptor} {associated_emoji}");

	let texture_creation_info = TextureCreationInfo::Text((
		Cow::Borrowed(inner_shared_state.font_info),

		TextDisplayInfo {
			text: DisplayText::new(&weather_string).with_padding("", " "),
			color: inner.text_color,
			pixel_area: params.area_drawn_to_screen,
			scroll_easer: easing_fns::scroll::LEFT_LINEAR,
			scroll_speed_multiplier: 1.0 / 3.0
		}
	));

	params.window.get_contents_mut().update_as_texture(
		true,
		params.texture_pool,
		&texture_creation_info,
		None,
		inner_shared_state.get_fallback_texture_creation_info
	)
}

pub async fn make_weather_window(
	top_left: Vec2f, size: Vec2f,
	update_rate_creator: UpdateRateCreator, api_key: &str,
	background_contents: WindowContents,
	text_color: ColorSDL, border_color: ColorSDL) -> GenericResult<Window> {

	let curr_location_json: serde_json::Value = request::as_type(request::get("https://ipinfo.io/json")).await?;

	let location = &curr_location_json["loc"].as_str().context("No location field available!")?;

	const UPDATE_RATE_SECS: Seconds = 60.0 * 10.0; // Once every 10 minutes

	// TODO: put the weather changes into the shared state, for proper on-screen error reporting; or remove all of the shared state somehow
	let weather_update_rate = update_rate_creator.new_instance(UPDATE_RATE_SECS);

	let request_url = request::build_url("https://api.tomorrow.io/v4/timelines",
		&[],

		&[
			("apikey", Cow::Borrowed(api_key)),
			("location", Cow::Borrowed(location)),
			("timesteps", Cow::Borrowed("1m")),
			("units", Cow::Borrowed("imperial")),
			("fields", Cow::Borrowed("temperature,weatherCode"))
		]
	);

	let data = WeatherStateData {
		text_color,
		request_url,
		weather_changed: false,
		curr_weather_info: None
	};

	let continually_updated = ContinuallyUpdated::new(&data, &(), "Weather").await;

	Ok(Window::new(
		Some((weather_updater_fn, weather_update_rate)),
		DynamicOptional::new(continually_updated),
		background_contents,
		Some(border_color),
		top_left,
		size,
		None
	))
}
