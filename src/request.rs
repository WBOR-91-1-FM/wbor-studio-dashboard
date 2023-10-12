use minreq;
use crate::generic_result::GenericResult;

fn check_request_failure<T: std::fmt::Display + std::cmp::PartialEq>
	(value_name: &str, url: &str, expected: T, gotten: T) -> GenericResult<()> {

	if expected != gotten {
		return Err(
			format!("Response {} for URL '{}' was not '{}', but '{}'",
			value_name, url, expected, gotten).into()
		)
	}

	Ok(())
}


pub fn get(url: &str) -> GenericResult<minreq::Response> {
	let response = minreq::get(url).send()?;

	check_request_failure("status code", url, 200, response.status_code)?;
	check_request_failure("reason phrase", url, "OK", &response.reason_phrase)?;

	Ok(response)
}
