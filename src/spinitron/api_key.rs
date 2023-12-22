use crate::utility_types::generic_result::GenericResult;

pub struct ApiKey {
	key: String
}

impl ApiKey {
	pub fn new() -> GenericResult<Self> {
		const API_KEY_PATH: &str = "assets/spinitron_api_key.txt";

		match std::fs::read_to_string(API_KEY_PATH) {
			Ok(untrimmed_api_key) => {
				Ok(Self {key: untrimmed_api_key.trim().to_string()})
			},
			Err(err) => {
				Err(format!("The API key at path '{}' could not be found. Official error: '{}'.",
					API_KEY_PATH, err).into())
			}
		}

	}

	pub fn get_inner_key(&self) -> String {
		self.key.clone()
	}
}
