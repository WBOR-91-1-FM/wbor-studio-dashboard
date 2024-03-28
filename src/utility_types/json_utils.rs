// TODO: put more in here

use crate::utility_types::generic_result::GenericResult;

pub fn load_from_file<T: for <'de> serde::Deserialize<'de>>(path: &str) -> GenericResult<T> {
	let file_contents = match std::fs::read_to_string(path) {
		Ok(contents) => Ok(contents),

		Err(err) => Err(format!(
			"The API key file at path '{path}' could not be found. Official error: '{err}'."
		))
	}?;

	Ok(serde_json::from_str(&file_contents)?)
}
