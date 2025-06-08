use std::{
	borrow::Cow,
	time::Duration
};

use reqwest::Client;
use tokio::sync::OnceCell;

use crate::utils::generic_result::*;

//////////

static CLIENT: OnceCell<Client> = OnceCell::const_new();
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10); // TODO: put this in a config file somewhere

//////////

type Response = GenericResult<reqwest::Response>;

pub fn init_client() {
	CLIENT.set(Client::builder().timeout(DEFAULT_TIMEOUT).build().unwrap()).unwrap();
}

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
	let client = CLIENT.get().unwrap();
	let mut request_builder = client.get(url);

	if let Some(header) = maybe_header {
		request_builder = request_builder.header(header.0, header.1);
	}

	let response = request_builder.send().await?;

	//////////

	let status = response.status();

	if status == reqwest::StatusCode::OK {
		Ok(response)
	}
	else {
		error_msg!("response status code for URL '{url}' was '{status}'")
	}
}

macro_rules! get_as {
	(@opt $header:expr) => {Some($header)};
	(@opt) => {None};

	($url:expr $(, $header:expr)?) => {{
		match $crate::utils::request::get($url, $crate::utils::request::get_as!(@opt $($header)?)).await {
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
				log::warn!("got an error from {description} URL #{}: '{err}'.", i + 1);
				continue;
			}
		};
	}

	error_msg!("none of the {description} URLs worked")
}
