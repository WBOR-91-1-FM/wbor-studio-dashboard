// This function is from here: https://easings.net/#easeOutBounce
fn ease_out_bounce(mut x: f64) -> f64 {
	let n1 = 7.5625;
	let d1 = 2.75;

	if x < 1.0 / d1 {
		n1 * x * x
	}
	else if x < 2.0 / d1 {
		x -= 1.5 / d1;
		n1 * x * x + 0.75
	}
	else if x < 2.5 / d1 {
		x -= 2.25 / d1;
		n1 * x * x + 0.9375
	}
	else {
		x -= 2.625 / d1;
		n1 * x * x + 0.984375
	}
}

//////////

pub mod scroll {
	use crate::texture::texture::TextTextureScrollEaser;

	pub const STAY_PUT: TextTextureScrollEaser = (|_, _| (0.0, true), 1.0);

	pub const PAUSE_THEN_SCROLL_LEFT: TextTextureScrollEaser = (
		|seed, text_fits_in_box| {
			if text_fits_in_box {return (0.0, true);}

			const TOTAL_CYCLE_TIME: f64 = 4.0;
			const SCROLL_TIME_PERCENT: f64 = 0.75;

			let wait_boundary = TOTAL_CYCLE_TIME * SCROLL_TIME_PERCENT;
			let scroll_value = seed % TOTAL_CYCLE_TIME;

			let scroll_fract = if scroll_value < wait_boundary {scroll_value / wait_boundary} else {0.0};
			(scroll_fract, true)
		},
		4.0
	);

	pub const OSCILLATE_NO_WRAP: TextTextureScrollEaser =
		(|seed, _| (seed.sin() * 0.5 + 0.5, false), std::f64::consts::TAU);

	pub const LEFT_LINEAR: TextTextureScrollEaser = (|seed, _| (seed % 1.0, true), 1.0);
}

//////////

pub mod transition {
	pub mod opacity {
		use crate::{
			texture::texture::TextureTransitionOpacityEaser,

			dashboard_defs::easing_fns::{
				ease_out_bounce,

				transition::aspect_ratio::{
					STRAIGHT_WAVY as AR_STRAIGHT_WAVY,
					JITTER_WAVY as AR_JITTER_WAVY
				}
			}
		};

		pub const LINEAR_BLENDED_FADE: TextureTransitionOpacityEaser = |percent_done| (1.0 - percent_done, percent_done);
		pub const BURST_BLENDED_FADE: TextureTransitionOpacityEaser = |percent_done| (0.0, percent_done);

		pub const LINEAR_BLENDED_BOUNCE: TextureTransitionOpacityEaser = |percent_done| LINEAR_BLENDED_FADE(ease_out_bounce(percent_done));
		pub const BURST_BLENDED_BOUNCE: TextureTransitionOpacityEaser = |percent_done| BURST_BLENDED_FADE(ease_out_bounce(percent_done));

		pub const STRAIGHT_WAVY: TextureTransitionOpacityEaser = |percent_done| {
			let y = AR_STRAIGHT_WAVY(percent_done);
			(1.0 - y, y)
		};

		pub const JITTER_WAVY: TextureTransitionOpacityEaser = |percent_done| {
			let y = AR_JITTER_WAVY(percent_done);
			(1.0 - y, y)
		};

		pub const FADE_OUT_THEN_FADE_IN: TextureTransitionOpacityEaser = |percent_done| {
			let twice_percent_done = percent_done * 2.0;
			if percent_done >= 0.5 {(0.0, twice_percent_done - 1.0)}
			else {(1.0 - twice_percent_done, 0.0)}
		};
	}

	pub mod aspect_ratio {
		use crate::{
			dashboard_defs::easing_fns::ease_out_bounce,
			texture::texture::TextureTransitionAspectRatioEaser
		};

		pub const LINEAR: TextureTransitionAspectRatioEaser = |percent_done| percent_done;
		pub const BOUNCE: TextureTransitionAspectRatioEaser = ease_out_bounce;

		pub const STRAIGHT_WAVY: TextureTransitionAspectRatioEaser = |percent_done| {
			if percent_done == 0.0 {return 0.0;}

			const N: u32 = 3; // This controls the frequency (must be odd)
			assert!(N % 2 != 0);

			let pi_n = std::f64::consts::PI * N as f64;
			(1.0 - (pi_n * percent_done).cos()) / (1.0 - pi_n.cos())
		};

		pub const JITTER_WAVY: TextureTransitionAspectRatioEaser = |percent_done| {
			const N: u32 = 3; // Must be an integer value

			let tau_n = std::f64::consts::TAU * N as f64;
			percent_done - ((tau_n * percent_done).sin() / tau_n)
		};
	}
}
