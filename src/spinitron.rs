// https://stackoverflow.com/questions/14154753/how-do-i-make-an-http-request-from-rust

use minreq;
use serde_json;
use serde;

type Uint = u32;
type MaybeBool = Option<bool>;
type MaybeUint = Option<Uint>;
type MaybeString = Option<String>;

// This does not cover all the spin fields; this is just the most useful subset of them.
#[derive(serde::Deserialize, Debug)] // TODO: remove `Debug`
pub struct Spin {
	////////// These are officially enabled fields

	artist: String,
	local: MaybeBool,
	song: String,

	// TODO: why is `time` not there?

	duration: Uint,
	request: MaybeBool,
	new: MaybeBool,

	release: String,

	va: MaybeBool,

	medium: MaybeString, // This should just be `String`, but it isn't here, for some reason
	released: MaybeUint

	////////// These are other fields

	// Ignoring "_links" for now. TODO: add start, end, image, and label later

	//////////
}

#[derive(serde::Deserialize, Debug)] // TODO: remove `Debug`
pub struct Persona {
	////////// These are fields that are officially supported by Spinitron

	id: Uint,
	name: String,

	bio: MaybeString,
	since: MaybeUint,

	email: String, // If there's no email, it will be `""`
	website: MaybeString, // If there's no website, it will be `None` or `Some("")`
	image: MaybeString, //  If there's no website, it will be `None`
}

fn do_get_request(url: &str) -> Result<minreq::Response, minreq::Error> {
	let response = minreq::get(url).send()?;
	assert_eq!(200, response.status_code);
	assert_eq!("OK", response.reason_phrase);
	Ok(response)
}

fn do_get_request_for_spinitron
	<T: for<'de> serde::Deserialize<'de>>
	(api_endpoint: &str, api_key: &str)
	-> Result<Vec<T>, Box<dyn std::error::Error>> {

	let url = format!("https://spinitron.com/api/{}?access-token={}", api_endpoint, api_key);

	let response = do_get_request(&url)?;
	let body = response.as_str()?;
	let parsed_json: serde_json::Value = serde_json::from_str(body)?;

	let parsed_json_as_object = parsed_json.as_object().unwrap();
	let requested_data_json = parsed_json_as_object["items"].as_array().unwrap();

	let final_data = requested_data_json.iter().map(|requested_datum_json|
		serde_json::from_value(requested_datum_json.clone()).unwrap()
	).collect();

	Ok(final_data)
}

/*
TODO: perhaps apply some spin filtering, if needed.
TODO: make fns for getting the current persona, show, and playlist.
TODO: maybe remove these wrappers, and use `do_get_request_for_spinitron` directly?
More API info here: https://spinitron.github.io/v2api/ */
pub fn get_recent_spins(api_key: &str) -> Result<Vec<Spin>, Box<dyn std::error::Error>> {
	do_get_request_for_spinitron("spins", api_key)
}

pub fn get_personas(api_key: &str) -> Result<Vec<Persona>, Box<dyn std::error::Error>> {
	do_get_request_for_spinitron("personas", api_key)
}

// TODO: figure out how to get the current persona id (maybe just ask for 1 persona?)
