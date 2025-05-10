use std::{
	borrow::Cow,
	time::Duration
};

use reqwest::Client;
use tokio::sync::OnceCell;

use crate::utility_types::generic_result::*;

//////////

static CLIENT: OnceCell<Client> = OnceCell::const_new();
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10); // TODO: put this in a config file somewhere

//////////

type Response = GenericResult<reqwest::Response>;

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

//////////

pub async fn get(url: &str, maybe_header: Option<(&str, &str)>) -> Response {
	let client = CLIENT.get_or_init(|| async {
		let build_client_sync = || reqwest::ClientBuilder::new().timeout(DEFAULT_TIMEOUT).build().unwrap();
		tokio::task::spawn_blocking(build_client_sync).await.unwrap()
	}).await;

	//////////

	let mut request_builder = client.get(url);

	if let Some(header) = maybe_header {
		request_builder = request_builder.header(header.0, header.1);
	}

	let response = request_builder.send().await?;

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

macro_rules! get_as {
	(@opt $header:expr) => {Some($header)};
	(@opt) => {None};

	($url:expr $(, $header:expr)?) => {{
		match $crate::request::get($url, $crate::request::get_as!(@opt $($header)?)).await {
			Ok(response) => response.json().await.to_generic_result(),
			Err(err) => Err(err)
		}
	}};
}

pub(crate) use get_as;

// TODO: how could I make it an exact-sized iterator, to print out the URL index over the total count?
pub async fn get_with_fallbacks_as<T: for<'de> serde::Deserialize<'de>, Url: AsRef<str>>
	(urls: impl Iterator<Item = Url>, description: &str) -> GenericResult<(T, Url)> {

	for (i, url) in urls.enumerate() {
		match get_as!(url.as_ref()) {
			Ok(result) => {
				return Ok((result, url));
			}

			Err(err) => {
				log::warn!("Got an error from {description} URL #{}: '{err}'.", i + 1);
				continue;
			}
		};
	}

	error_msg!("None of the {description} URLs worked!")
}
