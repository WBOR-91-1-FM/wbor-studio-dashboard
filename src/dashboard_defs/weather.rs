use std::{
	borrow::Cow,
	collections::HashMap
};

use crate::{
	texture::{
		text::{DisplayText, TextDisplayInfo},
		pool::{TextureCreationInfo, RemakeTransitionInfo}
	},

	utils::{
		time::*,
		request,
		generic_result::*,
		dynamic_optional::DynamicOptional,
		continually_updated::{ContinuallyUpdated, ContinuallyUpdatable}
	},

	window_tree::{
		Window,
		ColorSDL,
		WindowContents,
		TypicalWindowParams,
		WindowUpdaterParams
	},

	dashboard_defs::{
		easing_fns,
		shared_window_state::SharedWindowState
	}
};

// TODO: Should I do emoji-based forecasting work with the `ApiHistoryList` type?

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
	api_key: String,
	request_urls: Option<[Cow<'static, str>; 2]>, // This is loaded in asynchronously
	curr_weather_info: Vec<TimestampedWeatherInterval> // Info for different timestamps (TODO: make this an `Arc`?)
}

struct WeatherState {
	continually_updated: ContinuallyUpdated<WeatherApiState>,

	text_color: ColorSDL,
	weather_unit_symbol: char,
	curr_weather_interval: Option<(f32, &'static str, &'static str)>, // This is a weather interval, but without the timestamp
	maybe_remake_transition_info: Option<RemakeTransitionInfo>
}

async fn build_weather_request_urls(maybe_api_key: &str) -> GenericResult<[Cow<'static, str>; 2]> {
	#[derive(serde::Deserialize)]
	struct LocationInfo {
		loc: String
	}

	let location_info: LocationInfo = request::get_as!("https://ipinfo.io/json")?;

	// Here's the code behind the proxy: `https://github.com/WBOR-91-1-FM/wbor-weather-proxy`
	const PROXY_REQUEST_URL: &str = "https://api-2.wbor.org/weather";

	let fallback_request_url = request::build_url("https://api.tomorrow.io/v4/timelines",
		&[],

		&[
			("apikey", Cow::Borrowed(maybe_api_key)),
			("location", Cow::Borrowed(&location_info.loc)),
			("timesteps", Cow::Borrowed("1m")), // Timesteps of 1 minute, which is the highest allowed
			("units", Cow::Borrowed("imperial")), // Using degrees!
			("fields", Cow::Borrowed("temperature,weatherCode"))
		]
	);

	Ok([Cow::Borrowed(PROXY_REQUEST_URL), Cow::Owned(fallback_request_url)])
}

impl ContinuallyUpdatable for WeatherApiState {
	type Param = ();

	async fn update(&mut self, _: &Self::Param) -> MaybeError {
		// return Ok(()); // Use this line when developing locally, and you don't want to rate-limit this API in the studio!

		////////// If necessary, build the request URL from the API key (the API key is only supplied in the beginning).

		if self.request_urls.is_none() {
			self.request_urls = Some(build_weather_request_urls(&self.api_key).await?);
		}

		////////// Now, request the API

		let request_url_iterator = self.request_urls.as_ref().unwrap().iter();

		let (all_info_json, url): (serde_json::Value, _) = request::get_with_fallbacks_as(
			request_url_iterator, "weather info"
		).await?;

		// Note: the intervals are a series of weather predictions from this point on, spaced per some time amount.
		let intervals = &all_info_json["data"]["timelines"][0]["intervals"];

		self.curr_weather_info = intervals.as_array().unwrap().iter().filter_map(|interval| {
			let values = &interval["values"];

			let maybe_interval_fields = (
				interval["startTime"].as_str(), values["temperature"].as_f64(), values["weatherCode"].as_i64()
			);

			if let (Some(timestamp), Some(temperature), Some(associated_code)) = maybe_interval_fields {
				let (weather_code_descriptor, associated_emoji) = WEATHER_CODE_MAPPING.get(&(associated_code as u16)).unwrap();
				let timestamp: ReferenceTimestamp = parse_rfc3339_timestamp(timestamp).unwrap();
				Some((timestamp, (temperature as f32, *weather_code_descriptor, *associated_emoji)))
			}
			else {
				let conv_url = url.as_ref();

				// This happened once, and I don't know why. I'm trying to catch the bug like this!
				log::error!("The weather API didn't give back its needed fields, for some weird reason. \
					URL: '{conv_url}'. Fields: '{maybe_interval_fields:?}'. The whole interval: '{interval:?}'."
				);

				None
			}
		}).collect();

		Ok(())
	}
}

//////////

/* This updates the weather string displayed on the dashboard, given the prediction
data gathered every X minutes from the API. TODO: use the updatable text pattern here. */
fn weather_string_updater_fn(params: WindowUpdaterParams) -> MaybeError {
	let inner_shared_state = params.shared_window_state.get_mut::<SharedWindowState>();
	let there_are_already_window_contents = params.window.get_contents() != &WindowContents::Nothing;
	let individual_window_state = params.window.get_state_mut::<WeatherState>();

	individual_window_state.continually_updated.update((), &mut inner_shared_state.error_state);

	let api_state = individual_window_state.continually_updated.get_curr_data();
	let curr_time = get_reference_time();

	//////////

	/* The snippet below finds the closest weather prediction interval to the current time.

	TODO:
	- Maybe interpolate to update the weather even more in real-time?
	- Maybe just index directly into the interval array, assuming that enough entries exist,
		and that they're spaced equally timewise (sort the intervals before though)? */

	let maybe_closest_interval = api_state.curr_weather_info
		.iter().min_by_key(|(interval_time, ..)| {
			/* Getting the absolute value to get an absolute distance to the current time
			(just the closest prediction, regardless of whether it's from the past or the future) */
			let duration = interval_time.signed_duration_since(curr_time);
			duration.num_seconds().abs()
		}).map(|interval| interval.1);

	let interval_is_same_as_before = individual_window_state.curr_weather_interval == maybe_closest_interval;

	if interval_is_same_as_before {
		if there_are_already_window_contents {
			return Ok(()); // No need to update anything in this case
		}
		else {
			/* In this branch, there are no window contents yet (will only occur if there's no interval
			before, and no interval now, as well as no associated texture; i.e. on the first texture update,
			and the API was too slow to finish on its first iteration) */
		}
	}
	else {
		individual_window_state.curr_weather_interval = maybe_closest_interval;
	}

	//////////

	let weather_string = match maybe_closest_interval {
		Some((temperature, weather_code_descriptor, associated_emoji)) => {
			Cow::Owned(format!("Weather: {temperature}°{} and {weather_code_descriptor} {associated_emoji}", individual_window_state.weather_unit_symbol))
		}
		None => {
			Cow::Borrowed("No weather info available (yet)!")
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

pub fn make_weather_window(
	typical_params: TypicalWindowParams,
	api_key: &str, api_update_rate: Duration,

	text_color: ColorSDL,
	background_contents: WindowContents,
	maybe_remake_transition_info: Option<RemakeTransitionInfo>) -> Window {

	//////////

	let api_state = WeatherApiState {
		api_key: api_key.to_owned(),
		request_urls: None,
		curr_weather_info: Vec::new()
	};

	let continually_updated = ContinuallyUpdated::new(
		api_state, (), "Weather", api_update_rate
	);

	let weather_state = WeatherState {
		continually_updated,
		text_color,
		weather_unit_symbol: 'F',
		curr_weather_interval: None,
		maybe_remake_transition_info
	};

	//////////

	Window::new(
		vec![(weather_string_updater_fn, typical_params.view_refresh_update_rate)],
		DynamicOptional::new(weather_state),
		background_contents,
		typical_params.border_info,
		typical_params.top_left,
		typical_params.size,
		vec![]
	)
}
