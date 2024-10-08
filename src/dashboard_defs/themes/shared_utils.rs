use std::sync::atomic::{AtomicUsize, Ordering};

use crate::{
	error_msg,
	texture::TextureCreationInfo,
	utility_types::generic_result::*
};

//////////

#[derive(serde::Deserialize)]
pub struct ApiKeys {
	pub spinitron: String,
	pub tomorrow_io: String,
	pub twilio_account_sid: String,
	pub twilio_auth_token: String
}

//////////

static FALLBACK_TEXTURE_CREATION_INFO_PATH_INDEX: AtomicUsize = AtomicUsize::new(0);

lazy_static::lazy_static!(
	pub static ref FALLBACK_TEXTURE_PATHS: Vec<String> =
		std::fs::read_dir("assets/fallback_textures").unwrap()
		.map(|maybe_dir_entry| maybe_dir_entry.map(|dir_entry| {
			let path = dir_entry.path();
			assert!(path.is_file());
			path.to_str().unwrap().to_owned()
		}))
	   .collect::<Result<Vec<_>, _>>().unwrap();
);

pub fn get_fallback_texture_creation_info() -> TextureCreationInfo<'static> {
	let ordering = Ordering::SeqCst;
	let mut index = FALLBACK_TEXTURE_CREATION_INFO_PATH_INDEX.fetch_add(1, ordering);

	if index >= FALLBACK_TEXTURE_PATHS.len() {
		index = 0;
		FALLBACK_TEXTURE_CREATION_INFO_PATH_INDEX.store(0, ordering);
	}

	TextureCreationInfo::from_path(&FALLBACK_TEXTURE_PATHS[index])
}

//////////

// TODO: make this async once `async_std::process` is stabilized
pub fn run_command(command: &str, args: &[&str]) -> GenericResult<String> {
	let output = std::process::Command::new(command)
		.args(args)
		.output()?;

	if !output.status.success() {
		error_msg!("This command failed: '{command} {}'", args.join(" "))
	}
	else {
		String::from_utf8(output.stdout).to_generic().map(|s| s.trim().to_owned())
	}
}
