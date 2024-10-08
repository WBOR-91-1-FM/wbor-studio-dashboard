use crate::{
	window_tree::{
		Window,
		ColorSDL,
		GeneralLine,
		WindowContents,
		WindowUpdaterParams
	},

	utility_types::{
		vec2f::Vec2f,
		generic_result::*,
		update_rate::UpdateRate,
		dynamic_optional::DynamicOptional
	},

	dashboard_defs::shared_window_state::SharedWindowState
};

use chrono::{Local, Timelike};

// This is called raw because it's centered at (0, 0) and is unrotated.
type RawClockHand = GeneralLine<(f32, f32)>;

const NUM_CLOCK_HANDS: usize = 4;
const CLOCK_CENTER: (f32, f32) = (0.5, 0.5);

//////////

// These extents are defined assuming that the clock is pointing to 12:00
pub struct ClockHandConfig {
	x_extent: f32,
	minor_y_extent: f32,
	major_y_extent: f32,
	color: ColorSDL
}

impl ClockHandConfig {
	pub const fn new(x_extent: f32, minor_y_extent: f32, major_y_extent: f32, color: ColorSDL) -> Self {
		Self {x_extent, minor_y_extent, major_y_extent, color}
	}

	fn make_geometry(&self) -> RawClockHand {
		let hand = [
			// The minor part of the hand
			(-self.x_extent, 0.0),
			(0.0, self.minor_y_extent),
			(self.x_extent, 0.0),

			// The major part of the hand
			(0.0, -self.major_y_extent),
			(-self.x_extent, 0.0)
		];

		(self.color, hand.to_vec())
	}
}

pub struct ClockHandConfigs {
	pub milliseconds: ClockHandConfig,
	pub seconds: ClockHandConfig,
	pub minutes: ClockHandConfig,
	pub hours: ClockHandConfig
}

pub struct ClockHands {
	milliseconds: RawClockHand,
	seconds: RawClockHand,
	minutes: RawClockHand,
	hours: RawClockHand
}

impl ClockHands {
	pub fn new_with_window(
		update_rate: UpdateRate,
		top_left: Vec2f,
		size: Vec2f,
		hand_configs: ClockHandConfigs,
		dial_contents: WindowContents) -> GenericResult<(Self, Window)> {

		fn updater_fn(params: WindowUpdaterParams) -> MaybeError {
			let curr_time = Local::now();

			let time_units: [(u32, u32); NUM_CLOCK_HANDS] = [
				(curr_time.timestamp_subsec_millis(), 1000),
				(curr_time.second(), 60),
				(curr_time.minute(), 60),
				(curr_time.hour() % 12, 12)
			];

			let inner_shared_state = params.shared_window_state.get::<SharedWindowState>();
			let clock_hands = &inner_shared_state.clock_hands;

			let clock_hands_as_list: [&RawClockHand; NUM_CLOCK_HANDS] = [
				&clock_hands.milliseconds, &clock_hands.seconds, &clock_hands.minutes, &clock_hands.hours
			];

			//////////

			let WindowContents::Many(all_contents) = params.window.get_contents_mut()
			else {panic!("The clock's window contents was expected to be a list!")};

			let WindowContents::Lines(rotated_hands) = &mut all_contents[1]
			else {panic!("The second item in the clock's window contents was not a set of lines!")};

			//////////

			let mut prev_time_fract = 0.0;

			for ((raw_hand, rotated_hand), time_unit) in
				clock_hands_as_list.into_iter().zip(rotated_hands.iter_mut().rev()).zip(time_units) {

				let time_fract = (time_unit.0 as f32 + prev_time_fract) / time_unit.1 as f32;
				prev_time_fract = time_fract;

				let angle = time_fract * std::f32::consts::TAU;
				let (cos_angle, sin_angle) = (angle.cos(), angle.sin());

				rotated_hand.1.iter_mut().zip(&raw_hand.1).for_each(|(dest, raw)| {
					*dest = Vec2f::new(
						(raw.0 * cos_angle - raw.1 * sin_angle) + CLOCK_CENTER.0,
						(raw.0 * sin_angle + raw.1 * cos_angle) + CLOCK_CENTER.1
					);
				});
			}

			Ok(())
		}

		//////////

		let clock_hand_configs_as_list: [&ClockHandConfig; NUM_CLOCK_HANDS] = [
			&hand_configs.milliseconds, &hand_configs.seconds,
			&hand_configs.minutes, &hand_configs.hours
		];

		let raw_clock_hands = clock_hand_configs_as_list.map(|config| config.make_geometry());

		let line_contents = WindowContents::Lines(
			raw_clock_hands.iter().rev().map(|(color, clock_hand)| {
				(*color, vec![Vec2f::ZERO; clock_hand.len()])
			}).collect());

		let clock_window = Window::new(
			Some((updater_fn, update_rate)),
			DynamicOptional::NONE,
			WindowContents::Many(vec![dial_contents, line_contents]),
			None,
			top_left,
			size,
			None
		);

		Ok((
			ClockHands {
				milliseconds: raw_clock_hands[0].clone(),
				seconds: raw_clock_hands[1].clone(),
				minutes: raw_clock_hands[2].clone(),
				hours: raw_clock_hands[3].clone()
			},

			clock_window
		))
	}
}
