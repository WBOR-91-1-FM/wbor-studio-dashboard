/* TODO:
- Actually implement this
- Make the general structure of the text updater fns less repetitive
- Consider using an alternative API
*/

use std::borrow::Cow;

use crate::{
	// request,

	texture::{DisplayText, TextDisplayInfo, TextureCreationInfo},

	utility_types::{
		vec2f::Vec2f,
		generic_result::MaybeError,
		dynamic_optional::DynamicOptional,
		update_rate::{UpdateRateCreator, Seconds}
	},

	window_tree::{
		ColorSDL,
		Window,
		WindowContents,
		WindowUpdaterParams
	},

	dashboard_defs::shared_window_state::SharedWindowState
};

// TODO: fill this with stuff
struct WeatherWindowState {
	api_key: String,
	location: String
}

pub fn weather_updater_fn(params: WindowUpdaterParams) -> MaybeError {
	let weather_changed = true;
	let weather_string = "Rain (32f). So cold.";
	let weather_text_color = ColorSDL::BLACK;

	/*
	- 1000 API calls free every day
	- That's 1000 per 24 hrs
	- Our 41.666 per hour, or around once per 1.444 minutes
	- To make stuff easy, do once every 2 minutes
	- TODO: do it once every 10 minutes (that's how frequently the data updates: https://openweathermap.org/appid)
	*/

	// let individual_window_state = window.get_state::<WeatherWindowState>();
	let inner_shared_state = params.shared_window_state.get::<SharedWindowState>();

	/*
	// TODO: perhaps don't build request urls, just build request objects directly
	let url = request::build_url("https://api.openweathermap.org/data/2.5/weather",
		&[],

		&[
			("q", Cow::Borrowed(&individual_window_state.location)),
			("appid", Cow::Borrowed(&individual_window_state.api_key)),
			("units", Cow::Borrowed("metric"))
		]
	);

	//////////

	// TODO: why are all the damn fields optional?

	#[derive(serde::Deserialize, Debug)] // TODO: remove `Debug`
	struct WeatherDesc1 {
		feels_like: Option<f32>,
		temp: Option<f32>,
		pressure: Option<i32>,
		humidity: Option<i32>,
		temp_min: Option<f32>,
		temp_max: Option<f32>
	}

	#[derive(serde::Deserialize, Debug)] // TODO: remove `Debug`
	struct WeatherDesc2 {
		description: Option<String>,
		icon: Option<String>,
		id: Option<i32>,
		main: Option<String>
		// visibility: Option<i32>
	}

	#[derive(serde::Deserialize, Debug)] // TODO: remove `Debug`
	struct WindDesc {
		deg: Option<i32>,
		gust: Option<f32>,
		speed: Option<f32>
	}

	#[derive(serde::Deserialize, Debug)] // TODO: remove `Debug`
	struct CloudsDesc {
		all: Option<i32>
	}

	#[derive(serde::Deserialize, Debug)] // TODO: remove `Debug`
	struct RainDesc {
		// all: i32

		#[serde(rename = "1h")]
		one_hour: Option<f32>
	}

	#[derive(serde::Deserialize, Debug)] // TODO: remove `Debug`
	struct SnowDesc {
		all: Option<i32>
	}

	#[derive(serde::Deserialize, Debug)] // TODO: remove `Debug`
	struct WeatherInfo {
		main: WeatherDesc1,
		weather: [WeatherDesc2; 1],

		wind: Option<WindDesc>,
		clouds: Option<CloudsDesc>,
		rain: Option<RainDesc>,
		snow: Option<SnowDesc>
	}

	//////////

	let json = request::as_type(request::get(&url))?;
	let w: WeatherInfo = serde_json::from_value(json)?;

	fn maybe_add<T: std::fmt::Display>(string: &mut String, field: Option<T>, formatter: fn(T) -> String) {
		if let Some(inner) = field {
			*string += &(formatter(inner) + ". ");
		}
	}

	let mut weather_string = String::new();
	maybe_add(&mut weather_string, w.main.feels_like, |t| format!("It feels like {t}"));
	println!("ws = {:?}", weather_string);
	*/

	/*
	Deciding what data to show (I don't want to go overboard):
	1. (MOST IMPORTANT) An emoji for the given icon (I have this data)
	2. (SECOND-MOST IMPORTANT) What temperature it feels like
	3. (MID-LATER) If there's high pressure or humidity, say "It's a scorcher!" Or "It's a hot one today!".
	4. (LATER) If it's windy, show the wind gust and speed (same for rain, snow, etc.)
	*/

	let texture_creation_info = TextureCreationInfo::Text((
		Cow::Borrowed(inner_shared_state.font_info),

		TextDisplayInfo {
			text: DisplayText::new(weather_string),
			color: weather_text_color,
			pixel_area: params.area_drawn_to_screen,

			scroll_fn: |seed, _| {
				let repeat_rate_secs = 3.0;
				let base_scroll = (seed % repeat_rate_secs) / repeat_rate_secs;
				(1.0 - base_scroll, true)
			}
		}
	));

	params.window.get_contents_mut().update_as_texture(
		weather_changed,
		params.texture_pool,
		&texture_creation_info,
		inner_shared_state.fallback_texture_creation_info
	)
}

// Note: the state code can be empty here!
pub fn make_weather_window(
	top_left: Vec2f, size: Vec2f,
	update_rate_creator: UpdateRateCreator, api_key: &str,
	city_name: &str, state_code: &str, country_code: &str) -> Window {

	const UPDATE_RATE_SECS: Seconds = 60.0 * 10.0; // Once every 10 minutes (this is how frequent the weather data is)

	let weather_update_rate = update_rate_creator.new_instance(UPDATE_RATE_SECS);
	let location = [city_name, state_code, country_code].join(",");

	Window::new(
		Some((weather_updater_fn, weather_update_rate)),
		DynamicOptional::new(WeatherWindowState {api_key: api_key.to_string(), location}),
		WindowContents::Color(ColorSDL::RGB(255, 0, 255)),
		Some(ColorSDL::RED),
		top_left,
		size,
		None
	)
}
