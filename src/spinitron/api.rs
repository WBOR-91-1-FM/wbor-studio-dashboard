use std::borrow::Cow;

use serde::Deserialize;
use futures::TryFutureExt;

use crate::{
	request,
	utility_types::generic_result::*,

	spinitron::{
		wrapper_types::{SpinitronModelId, MaybeSpinitronModelId},
		model::{SpinitronModel, SpinitronModelWithProps, NUM_SPINITRON_MODEL_TYPES}
	}
};

/* TODO:
- Later on, maybe set up a mock API, for the sake of testing
- Would it be possible to show the current PSA on the dashboard?
- Fix the mysterious Serde-Spinitron-API error (that arose from a portion of the logs on the studio dashboard)
- Cache constructed Spinitron URLs (or whole requests) ahead of time
- Make the 2 util fns below compile-time constants somehow
*/

//////////

fn get_api_endpoint_from_spinitron_model<Model: SpinitronModel>() -> String {
	let full_typename: &str = std::any::type_name::<Model>();

	let last_colon_ind = full_typename.rfind(':').expect("Expected a colon in the model typename");
	let typename = &full_typename[last_colon_ind + 1..];

	let mut typename_chars = typename.chars();
	let first_char = typename_chars.next().expect("The typename has no chars in it, which is impossible");
	let api_endpoint = format!("{}{}s", first_char.to_lowercase(), &typename[1..]);

	const VALID_ENDPOINTS: [&str; NUM_SPINITRON_MODEL_TYPES] = ["spins", "playlists", "personas", "shows"];

	assert!(VALID_ENDPOINTS.contains(&api_endpoint.as_str()), "Invalid Spinitron API endpoint '{api_endpoint}'");

	api_endpoint
}

fn get_spinitron_model_fields<Model: SpinitronModelWithProps>() -> String {
	let default_model_as_serde_value = serde_json::to_value(Model::default()).expect("Could not serialize a default Spinitron model");

	let default_model_as_serde_obj = default_model_as_serde_value.as_object()
		.expect("Expected JSON to be an object for the default Spinitron model");

	// TODO: stop the `collect` allocation below
	let fields: Vec<&str> = default_model_as_serde_obj.iter().map(|(key, _)| key.as_str()).collect();
	fields.join(",")
}

//////////

// TODO: could inlining this reduce some future sizes?
async fn do_spinitron_request<Model: SpinitronModelWithProps, WrappingTheModel: for<'de> Deserialize<'de>>(
	api_key: &str, maybe_model_id: MaybeSpinitronModelId, maybe_item_count: Option<usize>) -> GenericResult<WrappingTheModel> {

	////////// Making some initial path and query params, and possibly adding a model id and item count to them

	let api_endpoint = get_api_endpoint_from_spinitron_model::<Model>();
	let joined_fields = get_spinitron_model_fields::<Model>();

	let mut path_params: Vec<Cow<str>> = vec![Cow::Borrowed(&api_endpoint)];

	let mut query_params: Vec<(&str, Cow<str>)> = vec![
		("access-token", Cow::Borrowed(api_key)),
		("fields", Cow::Borrowed(&joined_fields))
	];

	if let Some(model_id) = maybe_model_id {
		path_params.push(Cow::Owned(model_id.to_string()));
	}

	if let Some(item_count) = maybe_item_count {
		query_params.push(("count", Cow::Owned(item_count.to_string())));
	}

	////////// Building a URL, submitting the request, and getting the response JSON

	// Try the proxy URL first, and then try the standard Spinitron API URL
	const BASE_URLS: [&str; 2] = ["https://api-1.wbor.org/api", "https://spinitron.com/api"];

	// TODO: can I cache these constructed URLs later on, to avoid rebuilding them? Or, perhaps cache the underlying built request object...
	let urls_to_attempt = BASE_URLS.iter().map(|base_url| {
		let url = request::build_url(base_url, &path_params, &query_params);
		println!("Spinitron API URL: {url}");
		url
	});

	request::get_with_fallbacks_as(urls_to_attempt, "Spinitron").map_ok(|(model, _)| model).await
}

//////////

pub async fn get_model_from_id<T: SpinitronModelWithProps>(api_key: &str, id: SpinitronModelId) -> GenericResult<T> {
	// If requesting via a model id, just a raw item will be returned
	do_spinitron_request::<T, T>(api_key, Some(id), Some(1)).await
}

pub async fn get_most_recent_model<T: SpinitronModelWithProps>(api_key: &str) -> GenericResult<T> {
	#[derive(Deserialize)]
	struct ModelItemWrapper<T> {items: [T; 1]}

	// With no model ID, the first out of the one-entry `Vec` will be returned (not specifying an ID gets you an embedded `items` array)
	let response = do_spinitron_request::<T, ModelItemWrapper<T>>(
		api_key, None, Some(1)
	).await?;

	Ok(response.items[0].clone())
}

pub async fn get_models<T: SpinitronModelWithProps>(api_key: &str, maybe_item_count: Option<usize>) -> GenericResult<Vec<T>> {
	#[derive(Deserialize)]
	struct ModelItemsWrapper<T> {items: Vec<T>}

	let response = do_spinitron_request::<T, ModelItemsWrapper<T>>(
		api_key, None, maybe_item_count
	).await?;

	Ok(response.items)
}
