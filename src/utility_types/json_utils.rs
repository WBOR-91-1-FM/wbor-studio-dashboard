// TODO: put more in here

use crate::utility_types::generic_result::*;

pub async fn load_from_file<T: for <'de> serde::Deserialize<'de>>(path: &str) -> GenericResult<T> {
	let file_contents = match async_std::fs::read_to_string(path).await {
		Ok(contents) => Ok(contents),

		Err(err) => error_msg!(
			"The API key file at path '{path}' could not be found. Official error: '{err}'."
		)
	}?;

	serde_json::from_str(&file_contents).to_generic()
}
