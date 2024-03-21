use std::borrow::Cow;
use crate::utility_types::generic_result::GenericResult;

// TODO: use a string list concat fn in here somehow instead
pub fn build_url(base_url: &str, path_params: &[Cow<str>],
	query_params: &[(&str, Cow<str>)]) -> String {

	let mut url = String::new();

	url.push_str(base_url);

	for path_param in path_params {
		url.push('/');
		url.push_str(path_param);
	}

	for (index, (name, value)) in query_params.iter().enumerate() {
		url.push(if index == 0 {'?'} else {'&'});
		url.push_str(name);
		url.push('=');
		url.push_str(value);
	}

	url
}

/* TODO: in order to effectively do request stuff, maybe eliminate this wrapper
code altogether? Or just keep this wrapper layer as request submitting code? */
pub fn get_with_maybe_header(url: &str, maybe_header: Option<(&str, &str)>) -> GenericResult<minreq::Response> {
	const EXPECTED_STATUS_CODE: i32 = 200;
	const DEFAULT_TIMEOUT_SECONDS: u64 = 5;

	let mut request = minreq::get(url);

	if let Some(header) = maybe_header {
		request = request.with_header(header.0, header.1);
	}

	// TODO: make the app work when the network goes down temporarily
	let response = request.with_timeout(DEFAULT_TIMEOUT_SECONDS).send()?;

	if response.status_code == EXPECTED_STATUS_CODE {
		Ok(response)
	}
	else {
		Err(format!(
			"Response status code for URL '{url}' was not '{EXPECTED_STATUS_CODE}', \
			but '{}', with this reason: '{}'", response.status_code, response.reason_phrase
		).into())
	}
}

pub fn get(url: &str) -> GenericResult<minreq::Response> {
	get_with_maybe_header(url, None)
}

// This function is monadic!
pub fn as_type<T: for<'a> serde::Deserialize<'a>>(response: GenericResult<minreq::Response>) -> GenericResult<T> {
	let unpacked_response = response?;
	Ok(serde_json::from_str(unpacked_response.as_str()?)?)
}
