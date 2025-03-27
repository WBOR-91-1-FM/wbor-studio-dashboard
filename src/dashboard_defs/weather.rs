use std::{collections::HashMap, borrow::Cow};

use crate::{
	request,

	texture::{
		text::{DisplayText, TextDisplayInfo},
		pool::{TextureCreationInfo, RemakeTransitionInfo}
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

/*
TODO:
- Could emoji-based forecasting work with the `ApiHistoryList` type?
- Start using the weather proxy
- Make sure that `No weather available` updates don't result in a refresh
*/

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
This type constitutes a temperature, a weather code descriptor, and an associated emoji. */
type WeatherInterval = (f32, &'static str, &'static str);
type TimestampedWeatherInterval = (ReferenceTimestamp, WeatherInterval);

#[derive(Clone)] // This is used for the `ContinuallyUpdated` bit
struct WeatherApiState {
	request_urls: Option<[Cow<'static, str>; 2]>, // This is loaded in asynchronously
	curr_weather_info: Vec<TimestampedWeatherInterval> // Info for different timestamps
}

struct WeatherState {
	continually_updated: ContinuallyUpdated<WeatherApiState>,

	text_color: ColorSDL,
	weather_unit_symbol: char,
	curr_weather_interval: Option<(f32, &'static str, &'static str)>, // This is a weather interval, but without the timestamp
	maybe_remake_transition_info: Option<RemakeTransitionInfo>
}

impl Updatable for WeatherApiState {
	type Param = Option<String>; // The first time, this is the API key; the other times, it's nothing

	async fn update(&mut self, maybe_api_key: &Self::Param) -> MaybeError {
		// return Ok(()); // Use this line when developing locally, and you don't want to rate-limit this API in the studio!

		////////// If necessary, build the request URL from the API key (the API key is only supplied in the beginning).

		// Here's the code behind the proxy: `https://github.com/WBOR-91-1-FM/wbor-weather-proxy`
		const PROXY_REQUEST_URL: &str = "https://api-2.wbor.org/weather";

		if self.request_urls.is_none() {
			/* Unwrapping on this because:
			- The API key is only passed on the first call to this
			- If the API call failed and returned via `?`, the second time around, the API key will be `None` (which will lead to a second panic) */
			let curr_location_json: serde_json::Value = request::as_type(request::get("https://ipinfo.io/json")).await.unwrap();
			let location = &curr_location_json["loc"].as_str().expect("No location field available!");

			let fallback_request_url = request::build_url("https://api.tomorrow.io/v4/timelines",
				&[],

				&[
					("apikey", Cow::Borrowed(maybe_api_key.as_ref().unwrap())),
					("location", Cow::Borrowed(location)),
					("timesteps", Cow::Borrowed("1m")), // Timesteps of 1 minute, which is the highest allowed
					("units", Cow::Borrowed("imperial")), // Using degrees!
					("fields", Cow::Borrowed("temperature,weatherCode"))
				]
			);

			self.request_urls = Some([Cow::Borrowed(PROXY_REQUEST_URL), Cow::Owned(fallback_request_url)]);
		}

		////////// Now, request the API

		let request_urls = self.request_urls.as_ref().unwrap();
		let num_request_urls = request_urls.len();

		for (i, request_url) in request_urls.iter().enumerate() {
			let all_info_json: serde_json::Value = match request::as_type(request::get(request_url)).await {
				Ok(info) => info,

				Err(err) => {
					log::warn!("Could not get weather info from URL #{} (out of {num_request_urls}): '{err}'.", i + 1);
					continue;
				}
			};

			// Note: the intervals are a series of weather predictions from this point on, spaced per some time amount.
			let intervals = &all_info_json["data"]["timelines"][0]["intervals"];

			self.curr_weather_info = intervals.as_array().unwrap().iter().filter_map(|interval| {
				let values = &interval["values"];

				let maybe_interval_fields = (
					interval["startTime"].as_str(), values["temperature"].as_f64(), values["weatherCode"].as_i64()
				);

				if let (Some(timestamp), Some(temperature), Some(associated_code)) = maybe_interval_fields {
					let (weather_code_descriptor, associated_emoji) = WEATHER_CODE_MAPPING.get(&(associated_code as u16)).unwrap();
					let timestamp: ReferenceTimestamp = parse_time_from_rfc3339(timestamp).unwrap().into();
					Some((timestamp, (temperature as f32, *weather_code_descriptor, *associated_emoji)))
				}
				else {
					// This happened once, and I don't know why. I'm trying to catch the bug like this!
					log::error!("The weather API didn't give back the needed fields, for some weird reason. URL: '{request_url}'. Fields: {maybe_interval_fields:?}. The whole interval: {interval:?}.");
					None
				}
			}).collect();

			return Ok(());
		}

		error_msg!("None of the weather API URLs worked!")
	}
}

//////////

// This updates the weather API results every X minutes. Predictions are gathered for Y minutes forward.
fn weather_api_updater_fn(params: WindowUpdaterParams) -> MaybeError {
	let inner_shared_state = params.shared_window_state.get_mut::<SharedWindowState>();
	let individual_window_state = params.window.get_state_mut::<WeatherState>();
	individual_window_state.continually_updated.update(&None, &mut inner_shared_state.error_state);
	Ok(())
}

/* This updates the weather string displayed on the dashboard, given the prediction
data gathered every X minutes from the API. TODO: use the updatable text pattern here. */
fn realtime_weather_string_updater_fn(params: WindowUpdaterParams) -> MaybeError {
	let inner_shared_state = params.shared_window_state.get::<SharedWindowState>();
	let individual_window_state = params.window.get_state_mut::<WeatherState>();
	let api_state = individual_window_state.continually_updated.get_data();

	let weather_string = match api_state.curr_weather_info.len() {
		0 => "No weather info available!".to_owned(),

		_ => {
			let curr_time = get_reference_time();

			/* This snippet finds the closest weather prediction interval to the current time.
			TODO: perhaps interpolate to update the weather even more in real-time?
			TODO: perhaps just index directly into the interval array, assuming that enough entries exist
			for that to be possible, and that all of the intervals are spaced evenly timewise? */
			let closest_interval = api_state.curr_weather_info
				.iter().min_by_key(|(interval_time, ..)| {
					let duration = interval_time.signed_duration_since(curr_time);
					duration.num_seconds().abs()
				})
				.unwrap().1;

			//////////

			if individual_window_state.curr_weather_interval == Some(closest_interval) {
				return Ok(());
			}
			else {
				individual_window_state.curr_weather_interval = Some(closest_interval);
			}

			//////////

			let (temperature, weather_code_descriptor, associated_emoji) = closest_interval;
			format!("Weather: {temperature}°{} and {weather_code_descriptor} {associated_emoji}", individual_window_state.weather_unit_symbol)
		}
	};

	let texture_creation_info = TextureCreationInfo::Text((
		Cow::Borrowed(inner_shared_state.font_info),

		TextDisplayInfo::new(
			DisplayText::new(&weather_string).with_padding("", " "),
			individual_window_state.text_color,
			params.area_drawn_to_screen,
			easing_fns::scroll::LEFT_LINEAR,
			1.0 / 3.0
		)
	));

	let maybe_remake_transition_info = individual_window_state.maybe_remake_transition_info.clone();

	params.window.get_contents_mut().update_as_texture(
		true,
		params.texture_pool,
		&texture_creation_info,
		maybe_remake_transition_info.as_ref(),
		inner_shared_state.get_fallback_texture_creation_info
	)
}

pub async fn make_weather_window(
	api_key: &str, update_rate_creator: UpdateRateCreator,
	top_left: Vec2f, size: Vec2f,
	text_color: ColorSDL, border_color: ColorSDL,
	background_contents: WindowContents,
	maybe_remake_transition_info: Option<RemakeTransitionInfo>
) -> GenericResult<Window> {

	const API_UPDATE_RATE_SECS: Seconds = 60.0 * 10.0; // Once every 10 minutes
	const REFRESH_CURR_WEATHER_INFO_UPDATE_RATE_SECS: Seconds = 60.0; // Once per minute (this works in accordance with the timestep below)

	//////////

	let api_state = WeatherApiState {
		request_urls: None,
		curr_weather_info: Vec::new()
	};

	let api_key_param = Some(String::from(api_key));

	let weather_state = WeatherState {
		continually_updated: ContinuallyUpdated::new(api_state, api_key_param, "Weather").await,
		text_color,
		weather_unit_symbol: 'F',
		curr_weather_interval: None,
		maybe_remake_transition_info
	};

	//////////

	Ok(Window::new(
		vec![
			(weather_api_updater_fn, update_rate_creator.new_instance(API_UPDATE_RATE_SECS)),
			(realtime_weather_string_updater_fn, update_rate_creator.new_instance(REFRESH_CURR_WEATHER_INFO_UPDATE_RATE_SECS))
		],

		DynamicOptional::new(weather_state),
		background_contents,
		Some(border_color),
		top_left,
		size,
		vec![]
	))
}
