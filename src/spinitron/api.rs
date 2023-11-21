use serde_json;

use crate::{
	request,
	utility_types::generic_result::GenericResult,

	spinitron::{
		api_key::ApiKey,
		model::{SpinitronModelWithProps, Spin},
		wrapper_types::MaybeSpinitronModelId
	}
};

/* TODO:
- Later on, maybe set up a mock API, for the sake of testing
- Would it be possible to show the current PSA on the dashboard?
*/

fn get_json_from_spinitron_request<T: SpinitronModelWithProps>(
	api_key: &ApiKey, possible_model_id: MaybeSpinitronModelId,
	possible_item_count: Option<u16>
) -> GenericResult<serde_json::Value> {

	////////// Getting the API endpoint

	let full_typename = std::any::type_name::<T>();
	let last_colon_ind = full_typename.rfind(":").ok_or("Expected a colon in the model typename")?;
	let typename = &full_typename[last_colon_ind + 1..];

	let mut typename_chars = typename.chars();
	let first_char = typename_chars.nth(0).ok_or("The typename has no chars in it, which is impossible")?;
	let api_endpoint = format!("{}{}s", first_char.to_lowercase(), &typename[1..]);

	////////// Checking endpoint validity

	const VALID_ENDPOINTS: [&str; 4] = ["spins", "playlists", "personas", "shows"];

	if !VALID_ENDPOINTS.contains(&api_endpoint.as_str()) {
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

	// TODO: later on, cache this URL for the specific request (otherwise, a lot of time is spent rebuilding it)
	let url = request::build_url("https://spinitron.com/api", path_params, query_params)?;

	let response = request::get(&url)?;
	let body = response.as_str()?;

	Ok(serde_json::from_str(body)?)
}

fn get_vec_from_spinitron_json<T: SpinitronModelWithProps>(json: &serde_json::Value) -> GenericResult<Vec<T>> {
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
fn do_request<T: SpinitronModelWithProps>(api_key: &ApiKey, possible_model_id: MaybeSpinitronModelId) -> GenericResult<T> {
	let response_json = get_json_from_spinitron_request::<T>(api_key, possible_model_id, Some(1))?;

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

fn do_plural_request<T: SpinitronModelWithProps>(api_key: &ApiKey, possible_item_count: Option<u16>) -> GenericResult<Vec<T>> {
	let response_json = get_json_from_spinitron_request::<T>(api_key, None, possible_item_count)?;
	get_vec_from_spinitron_json(&response_json)
}

//////////

pub fn get_current_spin(api_key: &ApiKey) -> GenericResult<Spin> {
	do_request(api_key, None)
}

// TODO: can I make `id` non-optional?
pub fn get_from_id<T: SpinitronModelWithProps>(api_key: &ApiKey, id: MaybeSpinitronModelId) -> GenericResult<T> {
	do_request(api_key, id) // TODO: stop using this as a wrapper?
}
