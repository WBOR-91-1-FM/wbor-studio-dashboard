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

fn snappy(x: f64) -> f64 {
	if x < 0.3 {
		0.0
	}
	else {
		let p = (x - 0.3) / 0.7;
		let t = (1.0 - p).powi(8).min(1.0); // Doing the `min` to avoid overshoot
		1.0 - t
	}
}

fn rubber_band(x: f64) -> f64 {
	let freq = 4.0;
	let decay = 5.0;

	let oscillation = (x * std::f64::consts::PI * freq).sin();
	let envelope = (-x * decay).exp();
	(1.0 - oscillation * envelope).clamp(0.0, 1.0) // Doing the `clamp` to avoid under/overshoot
}

//////////

pub mod scroll {
	use crate::texture::text::TextTextureScrollEaser;

	pub const STAY_PUT: TextTextureScrollEaser = (|_, _, _| (0.0, true), 1.0);

	pub const LEFT_LINEAR: TextTextureScrollEaser = (|seed, _, _| {(seed, true)}, 1.0);

	pub const OSCILLATE_NO_WRAP: TextTextureScrollEaser =
		(|seed, _, _| (seed.sin() * 0.5 + 0.5, false), std::f64::consts::TAU);

	pub const PAUSE_THEN_SCROLL_LEFT: TextTextureScrollEaser = (
		|seed, period, text_fits_in_box| {
			if text_fits_in_box {return (0.0, true);}

			//////////

			const SCROLL_TIME_PERCENT: f64 = 0.75;

			let wait_boundary = period * SCROLL_TIME_PERCENT;

			let scroll_fract = if seed < wait_boundary {
				seed / wait_boundary
			}
			else {
				0.0
			};

			(scroll_fract, true)
		},
		4.0
	);
}

//////////

pub mod transition {
	pub mod opacity {
		use crate::{dashboard_defs::easing_fns::rubber_band, texture::pool::TextureTransitionOpacityEaser};

		use super::{
			super::{ease_out_bounce, snappy},
			aspect_ratio::{STRAIGHT_WAVY as AR_STRAIGHT_WAVY, JITTER_WAVY as AR_JITTER_WAVY}
		};

		//////////

		pub const LINEAR_BLENDED_FADE: TextureTransitionOpacityEaser = |percent_done| (1.0 - percent_done, percent_done);
		pub const BURST_BLENDED_FADE: TextureTransitionOpacityEaser = |percent_done| (0.0, percent_done);

		pub const LINEAR_BLENDED_BOUNCE: TextureTransitionOpacityEaser = |percent_done| LINEAR_BLENDED_FADE(ease_out_bounce(percent_done));
		pub const BURST_BLENDED_BOUNCE: TextureTransitionOpacityEaser = |percent_done| BURST_BLENDED_FADE(ease_out_bounce(percent_done));

		pub const LINEAR_BLENDED_SNAPPY: TextureTransitionOpacityEaser = |percent_done| LINEAR_BLENDED_FADE(snappy(percent_done));
		pub const BURST_BLENDED_SNAPPY: TextureTransitionOpacityEaser = |percent_done| BURST_BLENDED_FADE(snappy(percent_done));

		pub const LINEAR_BLENDED_RUBBER_BAND: TextureTransitionOpacityEaser = |percent_done| LINEAR_BLENDED_FADE(rubber_band(percent_done));
		pub const BURST_BLENDED_RUBBER_BAND: TextureTransitionOpacityEaser = |percent_done| BURST_BLENDED_FADE(rubber_band(percent_done));

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
		use super::super::{ease_out_bounce, snappy};
		use crate::{dashboard_defs::easing_fns::rubber_band, texture::pool::TextureTransitionAspectRatioEaser};

		pub const LINEAR: TextureTransitionAspectRatioEaser = |percent_done| percent_done;
		pub const BOUNCE: TextureTransitionAspectRatioEaser = ease_out_bounce;
		pub const SNAPPY: TextureTransitionAspectRatioEaser = snappy;
		pub const RUBBER_BAND: TextureTransitionAspectRatioEaser = rubber_band;

		pub const STRAIGHT_WAVY: TextureTransitionAspectRatioEaser = |percent_done| {
			const N: u32 = 3; // This controls the frequency (must be odd)
			const PI_N: f64 = std::f64::consts::PI * N as f64;

			// assert!(N % 2 != 0);

			let s = (PI_N * percent_done * 0.5).sin();
			s * s
		};

		pub const JITTER_WAVY: TextureTransitionAspectRatioEaser = |percent_done| {
			const N: u32 = 3;
			const TAU_N: f64 = std::f64::consts::TAU * N as f64;

			let y = percent_done - (TAU_N * percent_done).sin() / TAU_N;
			y.clamp(0.0, 1.0) // Floating-point errors can make it go slightly out of range
		};
	}
}

////////// Proofs that the easing functions are valid

macro_rules! generate_proof_group {
	(scroll, $mod:path, [$( $name:ident ),+ $(,)?]) => {
		mod scroll_proofs {
			$(
				#[cfg(kani)]
				#[kani::proof]
				#[allow(non_snake_case)]
				fn $name() {
					use $mod as easers;

					let (func, period) = easers::$name;

					assert!(period.is_finite(), "period is not finite");
					assert!(period > 0.0, "period is zero or negative");

					//////////

					let time_seed: f64 = kani::any();
					kani::assume(time_seed.is_finite() && (0.0..=period).contains(&time_seed));

					let (scroll_amount, _) = func(time_seed, period, kani::any());

					assert!(scroll_amount.is_finite(), "scroll amount is not finite");
					assert!((0.0..=1.0).contains(&scroll_amount), "scroll amount out of range");
				}
			)+
		}
	};

	(opacity, $mod:path, [$( $name:ident ),+ $(,)?]) => {
		mod opacity_proofs {
			$(
				#[cfg(kani)]
				#[kani::proof]
				#[allow(non_snake_case)]
				fn $name() {
					let x: f64 = kani::any();
					kani::assume(x.is_finite() && (0.0..=1.0).contains(&x));

					use $mod as easers;
					let (bg, fg) = easers::$name(x);

					assert!(bg.is_finite(), "background opacity is not finite");
					assert!(fg.is_finite(), "foreground opacity is not finite");
					assert!((0.0..=1.0).contains(&bg), "background opacity out of range");
					assert!((0.0..=1.0).contains(&fg), "foreground opacity out of range");
				}
			)+
		}
	};

	(aspect_ratio, $mod:path, [$( $name:ident ),+ $(,)?]) => {
		mod aspect_ratio_proofs {
			$(
				#[cfg(kani)]
				#[kani::proof]
				#[allow(non_snake_case)]
				fn $name() {
					let x: f64 = kani::any();
					kani::assume(x.is_finite() && (0.0..=1.0).contains(&x));

					use $mod as easers;
					let y = easers::$name(x);

					assert!(y.is_finite(), "aspect-ratio percent is not finite");
					assert!((0.0..=1.0).contains(&y), "aspect-ratio percent out of range");
				}
			)+
		}
	};
}

//////////

generate_proof_group!(
	scroll,
	crate::dashboard_defs::easing_fns::scroll,

	[STAY_PUT, LEFT_LINEAR, OSCILLATE_NO_WRAP, PAUSE_THEN_SCROLL_LEFT]
);

generate_proof_group!(
	opacity,
	crate::dashboard_defs::easing_fns::transition::opacity,

	[
		LINEAR_BLENDED_FADE, BURST_BLENDED_FADE,
		LINEAR_BLENDED_BOUNCE, BURST_BLENDED_BOUNCE,
		LINEAR_BLENDED_SNAPPY, BURST_BLENDED_SNAPPY,
		LINEAR_BLENDED_RUBBER_BAND, BURST_BLENDED_RUBBER_BAND,

		STRAIGHT_WAVY, JITTER_WAVY, FADE_OUT_THEN_FADE_IN
	]
);

generate_proof_group!(
	aspect_ratio,
	crate::dashboard_defs::easing_fns::transition::aspect_ratio,

	[LINEAR, BOUNCE, SNAPPY, RUBBER_BAND, STRAIGHT_WAVY, JITTER_WAVY]
);
