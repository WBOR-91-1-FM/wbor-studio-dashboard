use std::borrow::Cow;

use crate::{
	request,
	utility_types::generic_result::*,

	spinitron::{
		wrapper_types::MaybeSpinitronModelId,
		model::{SpinitronModelWithProps, NUM_SPINITRON_MODEL_TYPES}
	}
};

/* TODO:
- Later on, maybe set up a mock API, for the sake of testing
- Would it be possible to show the current PSA on the dashboard?
- Fix the mysterious Serde-Spinitron-API error (that arose from a portion of the logs on the studio dashboard)
*/

fn get_json_from_spinitron_request<T: SpinitronModelWithProps>(
	api_key: &str, possible_model_id: MaybeSpinitronModelId,
	possible_item_count: Option<u16>
) -> GenericResult<serde_json::Value> {

	////////// Getting the API endpoint

	let full_typename = std::any::type_name::<T>();
	let last_colon_ind = full_typename.rfind(':').context("Expected a colon in the model typename")?;
	let typename = &full_typename[last_colon_ind + 1..];

	let mut typename_chars = typename.chars();
	let first_char = typename_chars.next().context("The typename has no chars in it, which is impossible")?;
	let api_endpoint = format!("{}{}s", first_char.to_lowercase(), &typename[1..]);

	////////// Checking endpoint validity

	const VALID_ENDPOINTS: [&str; NUM_SPINITRON_MODEL_TYPES] = ["spins", "playlists", "personas", "shows"];

	if !VALID_ENDPOINTS.contains(&api_endpoint.as_str()) {
		return error_msg!("Invalid Spinitron API endpoint '{api_endpoint}'");
	}

	////////// Limiting the requested fields by what exists within the given model type

	let default_model_as_serde_value = serde_json::to_value(T::default())?;

	let default_model_as_serde_obj = default_model_as_serde_value.as_object()
		.context("Expected JSON to be an object for the default Spinitron model")?;

	// TODO: stop the `collect` allocation below
	let fields: Vec<&str> = default_model_as_serde_obj.iter().map(|(key, _)| key.as_str()).collect();
	let joined_fields = fields.join(",");

	////////// Making some initial path and query params, and possibly adding a model id and item count to them

	let mut path_params: Vec<Cow<str>> = vec![Cow::Owned(api_endpoint)];

	let mut query_params: Vec<(&str, Cow<str>)> = vec![
		("access-token", Cow::Owned(api_key.to_string())),
		("fields", Cow::Borrowed(&joined_fields))
	];

	if let Some(model_id) = possible_model_id {
		path_params.push(Cow::Owned(model_id.to_string()));
	}

	if let Some(item_count) = possible_item_count {
		query_params.push(("count", Cow::Owned(item_count.to_string())));
	}

	////////// Building a URL, submitting the request, and getting the response JSON

	/* TODO: later on, cache this URL for the specific request (otherwise, a lot of time is spent rebuilding it).
	Actually, don't do that, build the URL, and then cache the request itself (it will then be resent other times). */
	let url = request::build_url("https://spinitron.com/api", &path_params, &query_params);

	request::as_type(request::get(&url))
}

fn get_vec_from_spinitron_json<T: SpinitronModelWithProps>(json: &serde_json::Value) -> GenericResult<Vec<T>> {
	let parsed_json_as_object = json.as_object().context("Expected JSON to be an object")?;
	serde_json::from_value(parsed_json_as_object["items"].clone()).to_generic()
}

// This is a singular request
fn do_request<T: SpinitronModelWithProps>(api_key: &str, possible_model_id: MaybeSpinitronModelId) -> GenericResult<T> {
	let response_json = get_json_from_spinitron_request::<T>(api_key, possible_model_id, Some(1))?;

	if possible_model_id.is_some() {
		// If requesting a via model id, just a raw item will be returned
		serde_json::from_value(response_json).to_generic()
	}

	else {
		// Otherwise, the first out of the one-entry `Vec` will be returned
		let wrapped_in_vec: Vec<T> = get_vec_from_spinitron_json(&response_json)?;
		assert!(wrapped_in_vec.len() == 1);
		Ok(wrapped_in_vec[0].clone())
	}
}

/*
fn do_plural_request<T: SpinitronModelWithProps>(api_key: &str, possible_item_count: Option<u16>) -> GenericResult<Vec<T>> {
	let response_json = get_json_from_spinitron_request::<T>(api_key, None, possible_item_count)?;
	get_vec_from_spinitron_json(&response_json)
}
*/

//////////

// TODO: can I make `id` non-optional?
pub fn get_model_from_id<T: SpinitronModelWithProps>(api_key: &str, id: MaybeSpinitronModelId) -> GenericResult<T> {
	do_request(api_key, id) // TODO: stop using this as a wrapper?
}
