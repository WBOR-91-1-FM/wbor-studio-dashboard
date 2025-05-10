use rand::Rng;
use sdl2::render::BlendMode;
use tokio::process::Command;

use crate::{
	error_msg,
	window_tree::{Window, WindowContents},
	dashboard_defs::surprise::SurpriseCreationInfo,
	texture::pool::{TextureCreationInfo, TexturePool},

	utils::{
		file_utils,
		vec2f::Vec2f,
		time::Duration,
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

lazy_static::lazy_static!(
	static ref FALLBACK_TEXTURE_PATHS: Vec<String> = file_utils::read_filenames_from_directory("assets/fallback_textures");
);

pub const ALL_SURPRISES: [SurpriseCreationInfo; 8] = [
	SurpriseCreationInfo {
		texture_path: "assets/nathan.png",
		texture_blend_mode: BlendMode::None,

		update_rate: Duration::seconds(15),
		num_update_steps_to_appear_for: 1,
		chance_of_appearing_when_updating: 0.0007,

		local_hours_24_start: 8,
		local_hours_24_end: 22,

		flicker_window: false
	},

	SurpriseCreationInfo {
		texture_path: "assets/jumpscare.png",
		texture_blend_mode: BlendMode::Add,

		update_rate: Duration::milliseconds(35),
		num_update_steps_to_appear_for: 20,
		chance_of_appearing_when_updating: 0.0, // This one is only artificial too. Previous chance: `0.000002`

		local_hours_24_start: 0,
		local_hours_24_end: 23,

		flicker_window: true
	},

	SurpriseCreationInfo {
		texture_path: "assets/horrible.webp",
		texture_blend_mode: BlendMode::Add,

		update_rate: Duration::milliseconds(100),
		num_update_steps_to_appear_for: 9,
		chance_of_appearing_when_updating: 0.0, // This one can only be triggered artificially

		local_hours_24_start: 0,
		local_hours_24_end: 23,

		flicker_window: true
	},

	SurpriseCreationInfo {
		texture_path: "assets/hintze.jpg",
		texture_blend_mode: BlendMode::None,

		update_rate: Duration::milliseconds(800),
		num_update_steps_to_appear_for: 10,
		chance_of_appearing_when_updating: 0.00001,

		local_hours_24_start: 10,
		local_hours_24_end: 20,

		flicker_window: true
	},

	SurpriseCreationInfo {
		texture_path: "assets/poop.jpg",
		texture_blend_mode: BlendMode::None,

		update_rate: Duration::milliseconds(500),
		num_update_steps_to_appear_for: 8,
		chance_of_appearing_when_updating: 0.00001,

		local_hours_24_start: 0,
		local_hours_24_end: 23,

		flicker_window: true
	},

	SurpriseCreationInfo {
		texture_path: "assets/freaky_musk.jpg",
		texture_blend_mode: BlendMode::None,

		update_rate: Duration::milliseconds(1500),
		num_update_steps_to_appear_for: 4,
		chance_of_appearing_when_updating: 0.00004,

		// Musk being freaky is more of an evening thing
		local_hours_24_start: 18,
		local_hours_24_end: 23,

		flicker_window: true
	},

	SurpriseCreationInfo {
		texture_path: "assets/freaky_zuck.jpg",
		texture_blend_mode: BlendMode::None,

		update_rate: Duration::milliseconds(500),
		num_update_steps_to_appear_for: 12,
		chance_of_appearing_when_updating: 0.000013,

		// But Zuck starts early
		local_hours_24_start: 12,
		local_hours_24_end: 23,

		flicker_window: true
	},

	SurpriseCreationInfo {
		texture_path: "assets/jd_egg.png",
		texture_blend_mode: BlendMode::None,

		update_rate: Duration::seconds(1),
		num_update_steps_to_appear_for: 3,
		chance_of_appearing_when_updating: 0.00001,

		local_hours_24_start: 0,
		local_hours_24_end: 23,

		flicker_window: false
	}
];

//////////

pub fn get_fallback_texture_creation_info() -> TextureCreationInfo<'static> {
	let mut rand_generator = rand::thread_rng(); // TODO: can I cache this per each thread that uses it?
	let index = rand_generator.gen_range(0..FALLBACK_TEXTURE_PATHS.len());
	TextureCreationInfo::from_path(&FALLBACK_TEXTURE_PATHS[index])
}

//////////

pub async fn run_command(command: &str, args: &[&str]) -> GenericResult<String> {
	let output = Command::new(command)
		.args(args)
		.output().await?;

	if !output.status.success() {
		error_msg!("This command failed: '{command} {}'", args.join(" "))
	}
	else {
		String::from_utf8(output.stdout).map(|s| s.trim().to_owned()).to_generic_result()
	}
}

//////////

pub type StaticTextureSetInfo = [(&'static str, Vec2f, Vec2f, bool)];

pub async fn make_creation_info_for_static_texture_set(all_info: &StaticTextureSetInfo) -> GenericResult<Vec<TextureCreationInfo>> {
	TextureCreationInfo::from_paths_async(all_info.iter().map(|&(path, ..)| path)).await
}

pub fn add_static_texture_set(set: &mut Vec<Window>, all_info: &StaticTextureSetInfo,
	all_creation_info: &[TextureCreationInfo], texture_pool: &mut TexturePool) {

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
