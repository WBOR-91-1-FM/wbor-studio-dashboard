use std::sync::atomic::{AtomicUsize, Ordering};

use crate::{
	error_msg,
	window_tree::{Window, WindowContents},
	texture::pool::{TextureCreationInfo, TexturePool},

	utility_types::{
		file_utils,
		vec2f::Vec2f,
		generic_result::*,
		dynamic_optional::DynamicOptional
	}
};

//////////

#[derive(serde::Deserialize)]
pub struct ApiKeys {
	pub spinitron: String,
	pub twilio_account_sid: String,
	pub twilio_auth_token: String,
	pub tomorrow_io: String,
	pub streaming_server_now_playing_url: String // Not really an API key, but this is the best place to put it anyways
}

//////////

static FALLBACK_TEXTURE_CREATION_INFO_PATH_INDEX: AtomicUsize = AtomicUsize::new(0);

lazy_static::lazy_static!(
	static ref FALLBACK_TEXTURE_PATHS: Vec<String> = file_utils::read_filenames_from_directory("assets/fallback_textures");
);

//////////

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

//////////

pub type StaticTextureSetInfo = [(&'static str, Vec2f, Vec2f, bool)];

pub async fn make_creation_info_for_static_texture_set(all_info: &StaticTextureSetInfo) -> GenericResult<Vec<TextureCreationInfo>> {
	TextureCreationInfo::from_paths_async(all_info.iter().map(|&(path, ..)| path)).await
}

pub fn add_static_texture_set(set: &mut Vec<Window>, all_info: &StaticTextureSetInfo,
	all_creation_info: &[TextureCreationInfo<'_>], texture_pool: &mut TexturePool<'_>) {

	set.extend(all_info.iter().zip(all_creation_info).map(
		|(&(_, tl, size, skip_ar_correction), creation_info)| {

		let mut window = Window::new(
			vec![],
			DynamicOptional::NONE,
			WindowContents::make_texture_contents(creation_info, texture_pool).unwrap(),
			None,
			tl,
			size,
			vec![]
		);

		window.set_aspect_ratio_correction_skipping(skip_ar_correction);
		window
	}));
}
