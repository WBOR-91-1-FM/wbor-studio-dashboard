use crate::utility_types::generic_result::GenericResult;

pub struct ApiKey {
	key: String
}

impl ApiKey {
	pub fn new() -> GenericResult<Self> {
		let untrimmed_api_key = std::fs::read_to_string("assets/spinitron_api_key.txt")?;
		Ok(Self {key: untrimmed_api_key.trim().to_string()})
	}

	pub fn get_inner_key(&self) -> String {
		self.key.clone()
	}
}
