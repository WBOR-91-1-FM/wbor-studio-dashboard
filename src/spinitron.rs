// https://stackoverflow.com/questions/14154753/how-do-i-make-an-http-request-from-rust

use minreq;
use serde_json;
use serde;

// This does not cover all the spin fields; this is just the most useful subset of them.
#[derive(serde::Deserialize, Debug)] // TODO: remove `Debug`
pub struct Spin {
	////////// These are officially enabled fields

	artist: String,
	local: Option<bool>,
	song: String,

	// TODO: why is `time` not there?

	duration: u32,
	request: Option<bool>,
	new: Option<bool>,

	release: String,

	va: Option<bool>,

	medium: Option<String>, // This should just be `String`, but it isn't here, for some reason
	released: Option<u32>

	////////// These are other fields

	// Ignoring "_links" for now. TODO: add start, end, image, and label later

	//////////
}

pub fn do_get_request(url: &str) -> Result<minreq::Response, minreq::Error> {
	let response = minreq::get(url).send()?;
	assert_eq!(200, response.status_code);
	assert_eq!("OK", response.reason_phrase);
	Ok(response)
}

/* TODO: make fns for getting the current persona, show, and playlist.
More API info here: https://spinitron.github.io/v2api/ */
pub fn get_recent_spins(api_key: &str) -> Result<Vec<Spin>, Box<dyn std::error::Error>> {
	let url = format!("https://spinitron.com/api/spins?access-token={}", api_key);

	let response = do_get_request(&url)?;
	let body = response.as_str()?;
	let parsed_json: serde_json::Value = serde_json::from_str(body)?;

	let parsed_json_as_object = parsed_json.as_object().unwrap();
	let spins_json = parsed_json_as_object["items"].as_array().unwrap();

	let spins = spins_json.iter().map(|spin_json|
		serde_json::from_value(spin_json.clone()).unwrap()
	).collect();

	Ok(spins)
}
