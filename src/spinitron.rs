use serde;
use serde_json;

use crate::request;
use crate::texture;
use crate::texture::TextureHandle;
use crate::window_hierarchy::WindowContents;
use crate::generic_result::GenericResult;

////////// A wrapper type for API key creation

pub struct ApiKey {
	key: String
}

impl ApiKey {
	pub fn new() -> GenericResult<ApiKey> {
		let untrimmed_api_key: String = std::fs::read_to_string("assets/spinitron_api_key.txt")?;
		Ok(ApiKey {key: untrimmed_api_key.trim().to_string()})
	}
}

//////////

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

fn make_optional_api_arg<T: std::fmt::Debug>(arg_name: &str, arg: Option<T>) -> String {
	match arg {
		Some(inner_arg) => format!("&{}={:?}", arg_name, inner_arg),
		None => "".to_string()
	}
}

// TODO: eventually, avoid all possibilities of panics (so all assertions and unwraps should be gone)

fn do_get_request_for_spinitron<T: for<'de> serde::Deserialize<'de>>
	(api_endpoint: &str, api_key: &ApiKey, item_count: Option<u16>)
	-> GenericResult<Vec<T>> {

	const VALID_ENDPOINTS: [&str; 2] = ["spins", "personas"];

	if !VALID_ENDPOINTS.contains(&api_endpoint) {
		return Err(format!("Invalid Spinitron API endpoint '{}'", api_endpoint).into());
	}

	let url = format!("https://spinitron.com/api/{}?access-token={}{}",
		api_endpoint, api_key.key, make_optional_api_arg("count", item_count));

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

Also, here's an idea for getting the current info: get the current show with the `shows` endpoint,
then get all personas, then filter personas by if the current show matches, and then get the last spin.
Perhaps get personas faster by using the `playlists` endpoint, and get the last one, and then get the persona and show ids.
Can also get the last playlist by getting the current playlist id from the current spin.

More API info here: https://spinitron.github.io/v2api/ */
pub fn get_recent_spins(api_key: &ApiKey) -> GenericResult<Vec<Spin>> {
	do_get_request_for_spinitron("spins", api_key, None)
}

/* These are unordered. TODO: how do I get them in order of most recently played,
or just the most recent one? Also, is this all of the personas? It only returns around 20,
and I think that it should be more than that. */
pub fn get_personas(api_key: &ApiKey) -> GenericResult<Vec<Persona>> {
	do_get_request_for_spinitron("personas", api_key, None)
}

fn get_texture_from_optional_url(
	optional_url: &Option<String>,
	texture_pool: &mut texture::TexturePool)

	-> Option<GenericResult<TextureHandle>> {

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

	-> GenericResult<WindowContents> {

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
