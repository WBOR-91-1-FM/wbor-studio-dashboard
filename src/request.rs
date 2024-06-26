use std::borrow::Cow;
use crate::utility_types::generic_result::*;

pub fn build_url(base_url: &str, path_params: &[Cow<str>],
	query_params: &[(&str, Cow<str>)]) -> String {

	let mut url = base_url.to_string();

	for path_param in path_params {
		url += "/";
		url += path_param;
	}

	for (index, (name, value)) in query_params.iter().enumerate() {
		let sep = if index == 0 {"?"} else {"&"};
		for item in [sep, name, "=", value] {url += item;}
	}

	url
}

/* TODO: in order to effectively do request stuff, maybe eliminate this wrapper
code altogether? Or just keep this wrapper layer as request submitting code? */
pub fn get_with_maybe_header(url: &str, maybe_header: Option<(&str, &str)>) -> GenericResult<minreq::Response> {
	const EXPECTED_STATUS_CODE: i32 = 200;
	const DEFAULT_TIMEOUT_SECONDS: u64 = 20;

	let mut request = minreq::get(url);

	if let Some(header) = maybe_header {
		request = request.with_header(header.0, header.1);
	}

	let response = request.with_timeout(DEFAULT_TIMEOUT_SECONDS).send()?;

	if response.status_code == EXPECTED_STATUS_CODE {
		Ok(response)
	}
	else {
		error_msg!(
			"Response status code for URL '{url}' was not '{EXPECTED_STATUS_CODE}', \
			but '{}', with this reason: '{}'", response.status_code, response.reason_phrase
		)
	}
}

pub fn get(url: &str) -> GenericResult<minreq::Response> {
	get_with_maybe_header(url, None)
}

// This function is monadic!
pub fn as_type<T: for<'de> serde::Deserialize<'de>>(response: GenericResult<minreq::Response>) -> GenericResult<T> {
	let unpacked_response = response?;
	serde_json::from_str(unpacked_response.as_str()?).to_generic()
}
