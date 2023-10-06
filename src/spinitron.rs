// https://stackoverflow.com/questions/14154753/how-do-i-make-an-http-request-from-rust

use minreq;
use serde_json;
use serde;

use sdl2;

use crate::window_hierarchy::{TextureCreatorSDL, WindowContents};

type Uint = u32;
type MaybeBool = Option<bool>;
type MaybeUint = Option<Uint>;
type MaybeString = Option<String>;

// This does not cover all the spin fields; this is just the most useful subset of them.
#[derive(serde::Deserialize)]
pub struct Spin {
	////////// These are officially enabled fields

	artist: String,
	local: MaybeBool,
	song: String,

	// TODO: why is `time` not there?

	duration: Uint,
	request: MaybeBool,
	new: MaybeBool,

	release: String,

	va: MaybeBool,

	medium: MaybeString, // This should just be `String`, but it isn't here, for some reason
	released: MaybeUint,

	////////// These are other fields

	// Ignoring "_links" for now. TODO: add start, end, and label later

	pub image: MaybeString // If there's no image, it will be `None` or `Some("")`

	//////////
}

#[derive(serde::Deserialize)]
pub struct Persona {
	////////// These are fields that are officially supported by Spinitron

	id: Uint,
	name: String,

	bio: MaybeString,
	since: MaybeUint,

	email: String, // If there's no email, it will be `""`
	website: MaybeString, // If there's no website, it will be `None` or `Some("")`
	image: MaybeString, //  If there's no website, it will be `None`
}

fn do_get_request(url: &str) -> Result<minreq::Response, Box<dyn std::error::Error>> {
	fn check_request_failure<T: std::fmt::Display + std::cmp::PartialEq>
		(value_name: &str, url: &str, expected: T, gotten: T) -> Result<(), String>{

		if expected != gotten {
			return Err(
				format!("Response {} for URL '{}' was not '{}', but '{}'",
				value_name, url, expected, gotten)
			);
		}

		Ok(())
	}

	let response = minreq::get(url).send()?;

	check_request_failure("Response status code", url, 200, response.status_code)?;
	check_request_failure("reason phrase", url, "OK", &response.reason_phrase)?;

	Ok(response)
}

// TODO: eventually, avoid all possibilities of panics (so all assertions and unwraps should be gone)

fn do_get_request_for_spinitron
	<T: for<'de> serde::Deserialize<'de>>
	(api_endpoint: &str, api_key: &str)
	-> Result<Vec<T>, Box<dyn std::error::Error>> {

	const VALID_ENDPOINTS: [&str; 2] = ["spins", "personas"];

	if !VALID_ENDPOINTS.contains(&api_endpoint) {
		return Err(format!("Invalid Spinitron API endpoint '{}'", api_endpoint).into());
	}

	let url = format!("https://spinitron.com/api/{}?access-token={}", api_endpoint, api_key);

	let response = do_get_request(&url)?;
	let body = response.as_str()?;
	let parsed_json: serde_json::Value = serde_json::from_str(body)?;

	let parsed_json_as_object = parsed_json
		.as_object().ok_or("Expected JSON to be an object")?;

	let requested_data_json = parsed_json_as_object.get("items")
		.ok_or("Could not find key 'items' in Spinitron response JSON")?
		.as_array().ok_or("Expected Spinitron response JSON for key 'items' to be an array")?;

	Ok(requested_data_json.iter().map(|requested_datum_json|
		serde_json::from_value(requested_datum_json.clone()).unwrap()
	).collect())
}

/*
TODO: perhaps apply some spin filtering, if needed (e.g. only getting the last spin).
TODO: make fns for getting the current persona (from the set of all personas, or filter them), show, and playlist.
TODO: maybe remove these wrappers, and use `do_get_request_for_spinitron` directly?
More API info here: https://spinitron.github.io/v2api/ */
pub fn get_recent_spins<'a>(api_key: &str) -> Result<Vec<Spin>, Box<dyn std::error::Error>> {
	do_get_request_for_spinitron("spins", api_key)
}

pub fn get_personas(api_key: &str) -> Result<Vec<Persona>, Box<dyn std::error::Error>> {
	do_get_request_for_spinitron("personas", api_key)
}

// TODO: later on, examine texture allocation to see that textures are dropped when not needed
fn get_texture_from_url<'a>(url: &'a str, texture_creator: &'a TextureCreatorSDL)
	-> Result<sdl2::render::Texture<'a>, Box<dyn std::error::Error>> {

	use sdl2::image::LoadTexture;

	let request_result = do_get_request(url)?;
	Ok(texture_creator.load_texture_bytes(request_result.as_bytes())?)
}

/* TODO: don't copy the fallback contents (use reference counting?)
Or does this take ownership over it? */
pub fn get_curr_album_contents<'a>(spins: &'a Vec<Spin>,
	texture_creator: &'a TextureCreatorSDL,
	fallback_contents: WindowContents<'a>)

	-> Result<WindowContents<'a>, Box<dyn std::error::Error>> {

	if spins.len() > 0 {
		if let Some(url) = &spins[0].image {
			if !url.is_empty() {
				let texture = get_texture_from_url(url, texture_creator)?;
				return Ok(WindowContents::Texture(texture));
			}
		}
	}

	Ok(fallback_contents)
}
