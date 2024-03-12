use crate::utility_types::generic_result::GenericResult;

// TODO: use a string list concat fn in here somehow instead
pub fn build_url(base_url: &str, path_params: &[String],
	query_params: &[(&str, String)]) -> GenericResult<String> {

	let mut url = Vec::new();
	let mut add_str_to_url = |s: String| url.append(&mut s.into_bytes());

	//////////

	add_str_to_url(base_url.to_string());

	for path_param in path_params {
		add_str_to_url(format!("/{path_param}"));
	}

	for (index, (name, value)) in query_params.iter().enumerate() {
		let separator = if index == 0 {'?'} else {'&'};
		add_str_to_url(format!("{separator}{name}={value}"));
	}

	Ok(String::from_utf8(url)?)
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
