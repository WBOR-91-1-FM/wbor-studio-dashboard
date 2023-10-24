use serde_json;

use crate::request;
use crate::texture;
use crate::window_tree::WindowContents;
use crate::utility_types::generic_result::GenericResult;

use crate::spinitron::{
	api_key::ApiKey,
	wrapper_types::SpinitronModelId,
	model::{SpinitronModel, Spin, Playlist, Persona, Show}
};

// TODO: later on, maybe set up a mock API, for the sake of testing

fn get_json_from_spinitron_request<T: SpinitronModel>(
	api_endpoint: &str, api_key: &ApiKey,
	possible_model_id: Option<SpinitronModelId>,
	possible_item_count: Option<u16>
) -> GenericResult<serde_json::Value> {

	////////// Checking endpoint validity

	const VALID_ENDPOINTS: [&str; 4] = ["spins", "playlists", "personas", "shows"];

	if !VALID_ENDPOINTS.contains(&api_endpoint) {
		return Err(format!("Invalid Spinitron API endpoint '{}'", api_endpoint).into());
	}

	////////// Limiting the requested fields by what exists within the given model type

	let default_model_as_serde_value = serde_json::to_value(T::default())?;

	let default_model_as_serde_obj = default_model_as_serde_value.as_object()
		.ok_or("Expected JSON to be an object for the default Spinitron model")?;

	// TODO: stop the `collect` allocation below
	let fields: Vec<&str> = default_model_as_serde_obj.iter().map(|(key, _)| -> &str {key}).collect();
	let joined_fields = fields.join(",");

	////////// Making some initial path and query params, and possibly adding a model id and item count to them

	let mut path_params = vec![api_endpoint.to_string()];

	let mut query_params = vec![
		("access-token", api_key.get_inner_key()),
		("fields", joined_fields)
	];

	if let Some(model_id) = possible_model_id {
		path_params.push(model_id.to_string());
	}

	if let Some(item_count) = possible_item_count {
		query_params.push(("count", item_count.to_string()));
	}

	////////// Building a URL, submitting the request, and getting the response JSON

	// TODO: alter on, cache this URL, for the specific request (otherwise, a lot of time is spent rebuilding it)
	let url = request::build_url("https://spinitron.com/api", path_params, query_params)?;

	let response = request::get(&url)?;
	let body = response.as_str()?;

	Ok(serde_json::from_str(body)?)
}

fn get_vec_from_spinitron_json<T: SpinitronModel>(json: &serde_json::Value) -> GenericResult<Vec<T>> {
	let parsed_json_as_object = json.as_object().ok_or("Expected JSON to be an object")?;

	let requested_data_json = parsed_json_as_object.get("items")
		.ok_or("Could not find key 'items' in Spinitron response JSON")?
		.as_array().ok_or("Expected Spinitron response JSON for key 'items' to be an array")?;

	Ok(requested_data_json.iter().map(|requested_datum_json|
		// TODO: don't unwrap here
		serde_json::from_value(requested_datum_json.clone()).unwrap()
	).collect())
}

// This is a singular request
fn do_request<T: SpinitronModel>(api_endpoint: &str, api_key: &ApiKey,
	possible_model_id: Option<SpinitronModelId>) -> GenericResult<T> {

	let response_json = get_json_from_spinitron_request::<T>(
		api_endpoint, api_key, possible_model_id, Some(1))?;

	if possible_model_id.is_some() {
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

fn do_plural_request<T: SpinitronModel>(api_endpoint: &str, api_key: &ApiKey,
	possible_item_count: Option<u16>) -> GenericResult<Vec<T>> {

	let response_json = get_json_from_spinitron_request::<T>(
		api_endpoint, api_key, None, possible_item_count)?;

	get_vec_from_spinitron_json(&response_json)
}

////////// TODO: possibly remove some of these later on (they aren't terribly useful wrappers)

pub fn get_current_spin(api_key: &ApiKey) -> GenericResult<Spin> {
	do_request("spins", api_key, None)
}

pub fn get_playlist_from_id(api_key: &ApiKey, id: SpinitronModelId) -> GenericResult<Playlist> {
	do_request("playlists", api_key, Some(id))
}

pub fn get_persona_from_id(api_key: &ApiKey, id: SpinitronModelId) -> GenericResult<Persona> {
	do_request("personas", api_key, Some(id))
}

pub fn get_show_from_id(api_key: &ApiKey, possible_id: Option<SpinitronModelId>) -> GenericResult<Show> {
	do_request("shows", api_key, possible_id)
}

//////////

pub fn get_texture_from_optional_url(
	optional_url: &Option<String>,
	texture_pool: &mut texture::TexturePool)

	-> Option<GenericResult<WindowContents>> {

	if let Some(url) = &optional_url {
		if !url.is_empty() {
			let creation_info = texture::TextureCreationInfo::Path(url);
			let texture_handle = texture_pool.make_texture(creation_info);

			return Some(match texture_handle {
				Ok(inner_texture_handle) => Ok(WindowContents::Texture(inner_texture_handle)),
				Err(error) => Err(error)
			});
		}
	}

	None
}
