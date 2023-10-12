use minreq;

pub fn get(url: &str) -> Result<minreq::Response, Box<dyn std::error::Error>> {
	fn check_request_failure<T: std::fmt::Display + std::cmp::PartialEq>
		(value_name: &str, url: &str, expected: T, gotten: T) -> Result<(), String>{

		if expected != gotten {
			return Err(
				format!("Response {} for URL '{}' was not '{}', but '{}'",
				value_name, url, expected, gotten)
			);
		}

		Ok(())
	}

	let response = minreq::get(url).send()?;

	check_request_failure("status code", url, 200, response.status_code)?;
	check_request_failure("reason phrase", url, "OK", &response.reason_phrase)?;

	Ok(response)
}
