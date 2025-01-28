use std::{collections::HashMap, borrow::Cow};

use crate::{
	request,

	texture::{
		pool::TextureCreationInfo,
		text::{DisplayText, TextDisplayInfo}
	},

	utility_types::{
		time::*,
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

/* Note: an 'interval' is a piece of weather data predicted for some point into the future.
This type constitutes a timestamp, a temperature, a weather code descriptor, and an associated emoji. */
type WeatherIntervalDatum = (ReferenceTimestamp, f32, &'static str, &'static str);

#[derive(Clone)]
struct WeatherStateData {
	text_color: ColorSDL,

	request_url: String,
	weather_unit_symbol: char,

	// Info for different timestamps.
	curr_weather_info: Vec<WeatherIntervalDatum>
}

impl Updatable for WeatherStateData {
	type Param = ();

	async fn update(&mut self, _: &Self::Param) -> MaybeError {
		// return Ok(()); // Use this line when developing locally, and you don't want to rate-limit this API in the studio!

		let all_info_json: serde_json::Value = request::as_type(request::get(&self.request_url)).await?;

		// Note: the intervals are a series of weather predictions from this point on, spaced per some time amount.
		let intervals = &all_info_json["data"]["timelines"][0]["intervals"];

		// Unwrapping, just to make sure that critical errors resulting from here are never ignored
		self.curr_weather_info = intervals.as_array().unwrap().iter().map(|interval| {
			let values = &interval["values"];

			let (timestamp, temperature, associated_code) = (
				interval["startTime"].as_str().unwrap(),
				values["temperature"].as_f64().unwrap() as f32,
				values["weatherCode"].as_i64().unwrap() as u16
			);

			let (weather_code_descriptor, associated_emoji) = WEATHER_CODE_MAPPING.get(&(associated_code)).unwrap();
			let timestamp: ReferenceTimestamp = parse_time_from_rfc3339(timestamp).unwrap().into();

			(timestamp, temperature, *weather_code_descriptor, *associated_emoji)
		}).collect();

		Ok(())
	}
}

//////////

// This updates the weather API results every X minutes. Predictions are gathered for Y minutes forward.
fn weather_api_updater_fn(params: WindowUpdaterParams) -> MaybeError {
	let inner_shared_state = params.shared_window_state.get_mut::<SharedWindowState>();
	let individual_window_state = params.window.get_state_mut::<ContinuallyUpdated<WeatherStateData>>();
	individual_window_state.update(&(), &mut inner_shared_state.error_state)?;
	Ok(())
}

/* This updates the weather string displayed on the dashboard, given the prediction
data gathered every X minutes from the API. TODO: use the updatable text pattern here. */
fn realtime_weather_string_updater_fn(params: WindowUpdaterParams) -> MaybeError {
	let inner_shared_state = params.shared_window_state.get_mut::<SharedWindowState>();
	let individual_window_state = params.window.get_state_mut::<ContinuallyUpdated<WeatherStateData>>();
	let inner = individual_window_state.get_data();

	let weather_string = match inner.curr_weather_info.len() {
		0 => "No weather info available!".to_owned(),

		_ => {
			let curr_time = get_reference_time();

			/* This snippet finds the closest weather prediction interval to the current time.
			TODO: perhaps interpolate to update the weather even more in real-time?
			TODO: perhaps just index directly into the interval array, assuming that enough entries exist
			for that to be possible, and that all of the intervals are spaced evenly timewise? */
			let closest_interval = inner.curr_weather_info
				.iter().min_by_key(|(interval_time, ..)| {
					let duration = interval_time.signed_duration_since(curr_time);
					duration.num_seconds().abs()
				})
				.unwrap();

			let (temperature, weather_code_descriptor, associated_emoji) = (closest_interval.1, closest_interval.2, closest_interval.3);
			format!("Weather: {temperature}°{} and {weather_code_descriptor} {associated_emoji}", inner.weather_unit_symbol)
		}
	};

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

	const API_UPDATE_RATE_SECS: Seconds = 60.0 * 15.0; // Once every 15 minutes
	const REFRESH_CURR_WEATHER_INFO_UPDATE_RATE_SECS: Seconds = 60.0; // Once per minute (this works in accordance with the timestep below)

	let curr_location_json: serde_json::Value = request::as_type(request::get("https://ipinfo.io/json")).await?;
	let location = &curr_location_json["loc"].as_str().context("No location field available!")?;

	let request_url = request::build_url("https://api.tomorrow.io/v4/timelines",
		&[],

		&[
			("apikey", Cow::Borrowed(api_key)),
			("location", Cow::Borrowed(location)),
			("timesteps", Cow::Borrowed("1m")), // Timesteps of 1 minute, which is the highest allowed
			("units", Cow::Borrowed("imperial")), // Using degrees!
			("fields", Cow::Borrowed("temperature,weatherCode"))
		]
	);

	let data = WeatherStateData {
		text_color,
		request_url,
		weather_unit_symbol: 'F', // `F` for Fahrenheit (since we're using degrees)
		curr_weather_info: Vec::new()
	};

	let continually_updated = ContinuallyUpdated::new(&data, &(), "Weather").await;

	Ok(Window::new(
		vec![
			(weather_api_updater_fn, update_rate_creator.new_instance(API_UPDATE_RATE_SECS)),
			(realtime_weather_string_updater_fn, update_rate_creator.new_instance(REFRESH_CURR_WEATHER_INFO_UPDATE_RATE_SECS))
		],

		DynamicOptional::new(continually_updated),
		background_contents,
		Some(border_color),
		top_left,
		size,
		vec![]
	))
}
