use serde;
use serde_json;

use crate::request;
use crate::texture;
use crate::window_hierarchy::WindowContents;

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

	image: MaybeString // If there's no image, it will be `None` or `Some("")`

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

fn do_get_request_for_spinitron
	<T: for<'de> serde::Deserialize<'de>>
	(api_endpoint: &str, api_key: &str)
	-> Result<Vec<T>, Box<dyn std::error::Error>> {

	const VALID_ENDPOINTS: [&str; 2] = ["spins", "personas"];

	if !VALID_ENDPOINTS.contains(&api_endpoint) {
		return Err(format!("Invalid Spinitron API endpoint '{}'", api_endpoint).into());
	}

	let url = format!("https://spinitron.com/api/{}?access-token={}", api_endpoint, api_key);

	let response = request::get(&url)?;
	let body = response.as_str()?;
	let parsed_json: serde_json::Value = serde_json::from_str(body)?;

	let parsed_json_as_object = parsed_json
		.as_object().ok_or("Expected JSON to be an object")?;

	let requested_data_json = parsed_json_as_object.get("items")
		.ok_or("Could not find key 'items' in Spinitron response JSON")?
		.as_array().ok_or("Expected Spinitron response JSON for key 'items' to be an array")?;

	Ok(requested_data_json.iter().map(|requested_datum_json|
		// TODO: don't unwrap here
		serde_json::from_value(requested_datum_json.clone()).unwrap()
	).collect())
}

/* TODO:
- Perhaps apply some spin filtering, if needed (e.g. only getting the last spin).
- Make fns for getting the current persona (from the set of all personas, or filter them), show, and playlist.
- Maybe remove these wrappers, and use `do_get_request_for_spinitron` directly?

More API info here: https://spinitron.github.io/v2api/ */
pub fn get_recent_spins(api_key: &str) -> Result<Vec<Spin>, Box<dyn std::error::Error>> {
	do_get_request_for_spinitron("spins", api_key)
}

pub fn get_personas(api_key: &str) -> Result<Vec<Persona>, Box<dyn std::error::Error>> {
	do_get_request_for_spinitron("personas", api_key)
}

fn get_texture_from_optional_url(
	optional_url: &Option<String>,
	texture_pool: &mut texture::TexturePool)

	-> Option<Result<texture::TextureHandle, Box<dyn std::error::Error>>> {

	if let Some(url) = &optional_url {
		if !url.is_empty() {
			return Some(texture_pool.make_texture_from_url(url));
		}
	}

	return None
}

// TODO: make sure that this does not copy the fallback contents
pub fn get_curr_album_contents(
	spins: &Vec<Spin>,
	texture_pool: &mut texture::TexturePool,
	fallback_contents: WindowContents)

	-> Result<WindowContents, Box<dyn std::error::Error>> {

	if spins.len() > 0 {
		if let Some(texture) = get_texture_from_optional_url(&spins[0].image, texture_pool) {
			return match texture {
				Ok(inner_texture) => Ok(WindowContents::Texture(inner_texture)),
				Err(err) => Err(err)
			};
		}
	}

	Ok(fallback_contents)
}
