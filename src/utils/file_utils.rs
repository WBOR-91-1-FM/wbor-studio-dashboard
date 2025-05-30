use crate::utils::generic_result::*;

pub async fn read_file_contents(path: &str) -> GenericResult<Vec<u8>> {
	use tokio::io::AsyncReadExt;

	let mut file = match tokio::fs::File::open(path).await {
		Ok(file) => Ok(file),

		Err(err) => error_msg!(
			"The key file at path '{path}' could not be found. Official error: '{err}'."
		)
	}?;

	let mut contents = Vec::new();
	file.read_to_end(&mut contents).await?;
	Ok(contents)

}

pub async fn load_json_from_file<T: for <'de> serde::Deserialize<'de>>(path: &str) -> GenericResult<T> {
	let contents = read_file_contents(path).await?;
	serde_json::from_slice(&contents).to_generic_result()
}

pub fn read_filenames_from_directory(path: &str) -> Vec<String> {
	std::fs::read_dir(path).unwrap()
		.map(|maybe_dir_entry| maybe_dir_entry.map(|dir_entry| {
			let path = dir_entry.path();
			assert!(path.is_file());
			path.to_str().unwrap().to_owned()
		}))
	   .collect::<Result<Vec<_>, _>>().unwrap()
}
