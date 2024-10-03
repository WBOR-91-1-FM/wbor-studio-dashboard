use std::borrow::Cow;
use isahc::{config::Configurable, AsyncReadResponseExt};

use crate::utility_types::generic_result::*;

//////////

type Response = GenericResult<isahc::Response<isahc::AsyncBody>>;

pub fn build_url(base_url: &str, path_params: &[Cow<str>],
	query_params: &[(&str, Cow<str>)]) -> String {

	let mut url = base_url.to_owned();

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

pub async fn get_with_maybe_header(url: &str, maybe_header: Option<(&str, &str)>) -> Response {
	const DEFAULT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

	let mut request_builder = isahc::Request::get(url).timeout(DEFAULT_TIMEOUT);

	if let Some(header) = maybe_header {
		request_builder = request_builder.header(header.0, header.1);
	}

	let request = request_builder.body(())?;
	let response = isahc::send_async(request).await?;

	//////////

	let status = response.status();
	let status_code = status.as_u16();

	if status_code == 200 {
		Ok(response)
	}
	else {
		error_msg!(
			"Response status code for URL '{url}' was '{status_code}', with this reason: '{}'",
			status.canonical_reason().unwrap_or("unknown")
		)
	}
}

pub async fn get(url: &str) -> Response {
	get_with_maybe_header(url, None).await
}

// This function is monadic!
pub async fn as_type<T: for<'de> serde::Deserialize<'de>>(response: impl std::future::Future<Output = Response>) -> GenericResult<T> {
	let mut unpacked_response = response.await?;
	let text = unpacked_response.text().await?;
	serde_json::from_str(&text).to_generic()
}
