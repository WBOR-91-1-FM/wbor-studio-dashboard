use serde;
use serde_json;

use crate::request;
use crate::texture;
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

////////// Some useful wrapper types

type Uint = u32;
type Bool = bool;

type MaybeBool = Option<bool>;
type MaybeUint = Option<Uint>;
type MaybeString = Option<String>;

type SpinitronModelId = u32;

////////// The spinitron model types

#[allow(dead_code)] // TODO: remove
#[derive(serde::Deserialize, Clone, Debug)] // TODO: remove `Debug`
pub struct Spin {
	// This does not cover all the spin fields; this is just the most useful subset of them.

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

	playlist_id: Uint,
	image: MaybeString // If there's no image, it will be `None` or `Some("")`
}

#[allow(dead_code)] // TODO: remove
#[derive(serde::Deserialize, Clone, Debug)] // TODO: remove `Debug`
pub struct Playlist {
	id: Uint,
	persona_id: Uint, // TODO: why are all the persona ids the same?
	show_id: MaybeUint, // TODO: why is this optional?

	start: String,
	end: String,
	duration: Uint,
	timezone: String,

	category: MaybeString,
	title: String,
	description: MaybeString,
	since: MaybeUint,

	url: MaybeString, // TODO: maybe remove this
	hide_dj: MaybeUint, // 0 or 1
	image: MaybeString,
	automation: MaybeUint, // 0 or 1

	episode_name: MaybeString,
	episode_description: MaybeString
}

#[allow(dead_code)] // TODO: remove
#[derive(serde::Deserialize, Clone, Debug)] // TODO: remove `Debug`
pub struct Persona {
	////////// These are fields that are officially supported by Spinitron

	id: Uint,
	name: String,

	bio: MaybeString,
	since: MaybeUint,

	email: String, // If there's no email, it will be `""`
	website: MaybeString, // If there's no website, it will be `None` or `Some("")`
	image: MaybeString //  If there's no website, it will be `None`
}

#[allow(dead_code)] // TODO: remove
#[derive(serde::Deserialize, Clone, Debug)] // TODO: remove `Debug`
pub struct Show {
	id: Uint,

	start: String,
	end: String,
	duration: Uint,
	timezone: String,

	one_off: Bool,

	category: String,
	title: String,
	description: String,

	since: MaybeUint,
	url: String,
	hide_dj: Uint, // 0 or 1
	image: MaybeString
}

//////////

fn get_json_from_spinitron_request(
	api_endpoint: &str, api_key: &ApiKey,
	possible_model_id: Option<SpinitronModelId>,
	possible_item_count: Option<u16>) -> GenericResult<serde_json::Value> {

	////////// Checking endpoint validity

	const VALID_ENDPOINTS: [&str; 4] = ["spins", "playlists", "personas", "shows"];

	if !VALID_ENDPOINTS.contains(&api_endpoint) {
		return Err(format!("Invalid Spinitron API endpoint '{}'", api_endpoint).into());
	}

	////////// Making a request URL

	let mut path_params = vec![api_endpoint.to_string()];
	let mut query_params = vec![("access-token", api_key.key.to_string())];

	if let Some(model_id) = possible_model_id {
		path_params.push(model_id.to_string());
	}

	if let Some(item_count) = possible_item_count {
		query_params.push(("count", item_count.to_string()));
	}

	let url = request::build_url("https://spinitron.com/api", path_params, query_params)?;

	////////// Submitting the request, and getting JSON from it

	let response = request::get(&url)?;
	let body = response.as_str()?;

	Ok(serde_json::from_str(body)?)
}

fn get_vec_from_spinitron_json<T: for<'de> serde::Deserialize<'de>>(json: &serde_json::Value) -> GenericResult<Vec<T>> {
	let parsed_json_as_object = json.as_object().ok_or("Expected JSON to be an object")?;

	let requested_data_json = parsed_json_as_object.get("items")
		.ok_or("Could not find key 'items' in Spinitron response JSON")?
		.as_array().ok_or("Expected Spinitron response JSON for key 'items' to be an array")?;

	Ok(requested_data_json.iter().map(|requested_datum_json|
		// TODO: don't unwrap here
		serde_json::from_value(requested_datum_json.clone()).unwrap()
	).collect())
}

fn do_singular_spinitron_request<T: for<'de> serde::Deserialize<'de> + Clone>
	(api_endpoint: &str, api_key: &ApiKey, possible_model_id: Option<SpinitronModelId>) -> GenericResult<T> {

	let response_json = get_json_from_spinitron_request(
		api_endpoint, api_key, possible_model_id, Some(1))?;

	if let Some(_) = possible_model_id {
		// If requesting a via model id, just a raw item will be returned
		Ok(serde_json::from_value(response_json)?)
	}

	else {
		// Otherwise, the first out of the one-entry `Vec` will be returned
		let wrapped_in_vec: Vec<T> = get_vec_from_spinitron_json(&response_json)?;
		// assert!(wrapped_in_vec.len() == 1);
		Ok(wrapped_in_vec[0].clone())
	}
}

fn do_plural_spinitron_request<T: for<'de> serde::Deserialize<'de>>(api_endpoint: &str,
	api_key: &ApiKey, possible_item_count: Option<u16>) -> GenericResult<Vec<T>> {

	let response_json = get_json_from_spinitron_request(
		api_endpoint, api_key, None, possible_item_count)?;

	get_vec_from_spinitron_json(&response_json)
}

//////////

/* These are unordered. TODO: how do I get them in order of most recently played,
or just the most recent one? Also, is this all of the personas? It only returns around 20,
and I think that it should be more than that. */

fn get_current_spin(api_key: &ApiKey) -> GenericResult<Spin> {
	do_singular_spinitron_request("spins", api_key, None)
}

fn get_playlist_from_id(api_key: &ApiKey, id: SpinitronModelId) -> GenericResult<Playlist> {
	do_singular_spinitron_request("playlists", api_key, Some(id))
}

fn get_persona_from_id(api_key: &ApiKey, id: SpinitronModelId) -> GenericResult<Persona> {
	do_singular_spinitron_request("personas", api_key, Some(id))
}

fn get_show_from_id(api_key: &ApiKey, possible_id: Option<SpinitronModelId>) -> GenericResult<Option<Show>> {
	if let None = possible_id {
		return Ok(None)
	}

	do_singular_spinitron_request("shows", api_key, possible_id)
}

/* TODO: later on, if the current playlist and persona ids
are the same, don't send requests for them again */
pub fn get_current_data(api_key: &ApiKey) -> GenericResult<(Spin, Playlist, Persona, Option<Show>)> {
	let current_spin = get_current_spin(api_key)?;
	let current_playlist = get_playlist_from_id(api_key, current_spin.playlist_id)?;
	let current_persona = get_persona_from_id(api_key, current_playlist.persona_id)?;
	let current_show = get_show_from_id(api_key, current_playlist.show_id)?;

	Ok((current_spin, current_playlist, current_persona, current_show))
}

fn get_texture_from_optional_url(
	optional_url: &Option<String>,
	texture_pool: &mut texture::TexturePool)

	-> Option<GenericResult<texture::TextureHandle>> {

	if let Some(url) = &optional_url {
		if !url.is_empty() {
			return Some(texture_pool.make_texture_from_url(url));
		}
	}

	return None
}

// TODO: make sure that this does not copy the fallback contents
pub fn get_current_album_contents(
	current_spin: &Spin,
	texture_pool: &mut texture::TexturePool,
	fallback_contents: WindowContents)

	-> GenericResult<WindowContents> {

	if let Some(texture) = get_texture_from_optional_url(&current_spin.image, texture_pool) {
		return match texture {
			Ok(inner_texture) => Ok(WindowContents::Texture(inner_texture)),
			Err(err) => Err(err)
		};
	}

	Ok(fallback_contents)
}
