use rand::Rng;
use std::borrow::Cow;

use crate::{
	utility_types::{
		time::Duration,
		generic_result::*
	},

	texture::{
		TexturePool,
		TextureCreationInfo,
		RemakeTransitionInfo,
		TextureTransitionOpacityEaser,
		TextureTransitionAspectRatioEaser
	},

	dashboard_defs::easing_fns,
	window_tree::WindowContents
};

pub struct IntermediateTextureTransitionInfo {
	pub percent_chance_to_show_rand_intermediate_texture: f64,
	pub rand_duration_range_for_intermediate: (f64, f64),
	pub max_random_transitions: usize
}

fn pick_from_slice<'a, T>(choices: &'a [T], rand_generator: &mut rand::rngs::ThreadRng) -> &'a T {
	&choices[rand_generator.gen_range(0..choices.len())]
}

fn pick_random_easing_pair(rand_generator: &mut rand::rngs::ThreadRng)
	-> (TextureTransitionOpacityEaser, TextureTransitionAspectRatioEaser) {

	use easing_fns::transition::{opacity, aspect_ratio};

	let easing_pairs = [
		(opacity::LINEAR_BLENDED_FADE, aspect_ratio::LINEAR),
		(opacity::BURST_BLENDED_FADE, aspect_ratio::LINEAR),
		(opacity::FADE_OUT_THEN_FADE_IN, aspect_ratio::LINEAR),

		(opacity::LINEAR_BLENDED_BOUNCE, aspect_ratio::BOUNCE),
		(opacity::BURST_BLENDED_BOUNCE, aspect_ratio::BOUNCE),

		(opacity::STRAIGHT_WAVY, aspect_ratio::STRAIGHT_WAVY),
		(opacity::JITTER_WAVY, aspect_ratio::JITTER_WAVY)
	];

	*pick_from_slice(&easing_pairs, rand_generator)
}

pub fn update_as_texture_with_funky_remake_transition(
	window_contents: &mut WindowContents,
	texture_pool: &mut TexturePool,
	texture_creation_info: &TextureCreationInfo,
	duration: Duration,
	rand_generator: &mut rand::rngs::ThreadRng,
	get_fallback_texture_creation_info: fn() -> TextureCreationInfo<'static>,
	mut intermediate_info: IntermediateTextureTransitionInfo) -> MaybeError {

	//////////

	let path = |base| TextureCreationInfo::Path(Cow::Borrowed(base));

	let all_intermediate_texture_creation_info = [
		path("assets/business_expert.jpg"),
		path("assets/house_1.jpg"),
		path("assets/house_2.jpg"),

		path("assets/bird.jpg"),
		path("assets/brock.png"),
		path("assets/dog.png"),
		path("assets/horse_cop.png"),
		path("assets/monks.png"),
		path("assets/polar_board.png"),
		path("assets/putin_kim.png"),

		path("assets/willem.png"),
		path("assets/deer_1.png"),
		path("assets/deer_2.png"),
		path("assets/waltuh.png"),
		path("assets/bird_boots.png"),
		path("assets/lobster.png")
	];

	//////////

	// Randomly recurring to show intermediate textures before the final one
	if  intermediate_info.max_random_transitions != 0 &&
		rand_generator.gen_range(0.0..1.0) < intermediate_info.percent_chance_to_show_rand_intermediate_texture {

		let intermediate_texture_creation_info = pick_from_slice(&all_intermediate_texture_creation_info, rand_generator);

		let range = intermediate_info.rand_duration_range_for_intermediate;
		let rand_duration_secs = rand_generator.gen_range(range.0..range.1);
		let rand_duration_ms = (rand_duration_secs * 1000.0) as i64;

		intermediate_info.max_random_transitions -= 1;

		update_as_texture_with_funky_remake_transition(
			window_contents, texture_pool, intermediate_texture_creation_info,
			Duration::milliseconds(rand_duration_ms),
			rand_generator,
			get_fallback_texture_creation_info,
			intermediate_info
		)?;
	}

	////////// Making a remake transition

	let easers = pick_random_easing_pair(rand_generator);

	let remake_transition_info = RemakeTransitionInfo::new(
		duration, easers.0, easers.1
	);

	////////// Updating

	window_contents.update_as_texture(
		true,
		texture_pool,
		texture_creation_info,
		Some(&remake_transition_info),
		get_fallback_texture_creation_info
	)
}
