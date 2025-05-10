use std::num::Wrapping;

pub type Seconds = f64;
type FrameIndex = u32; // Intended to wrap, so no bigger type is needed
type Fps = u32;

//////////

#[derive(Copy, Clone)]
pub struct UpdateRate {
	every_n_frames: FrameIndex
}

impl UpdateRate {
	pub const ONCE_PER_FRAME: Self = Self {every_n_frames: 1};

	// TODO: make an option for only running something once instead
	pub const ALMOST_NEVER: Self = Self {every_n_frames: FrameIndex::MAX};

	fn new(num_seconds_between_updates: Seconds, fps: Fps) -> Self {
		let max_frame_index = FrameIndex::MAX;

		let num_frames_between_updates = num_seconds_between_updates * fps as Seconds;

		let report_update_rate_error =
			|below_or_above_str, min_or_max_str, boundary| panic!(
				"{num_seconds_between_updates} seconds between updates yields {num_frames_between_updates} \
				frames between updates (rounded), which is {below_or_above_str} the allowed {min_or_max_str} of {boundary}"
			);

		if num_frames_between_updates < 1.0 {
			report_update_rate_error("below", "minimum", "1")
		}
		else if num_frames_between_updates > max_frame_index as Seconds {
			report_update_rate_error("above", "maximum", &max_frame_index.to_string());
		}

		//////////

		// This is floored
		Self {every_n_frames: num_frames_between_updates as FrameIndex}
	}

	pub const fn is_time_to_update(self, frame_counter: FrameCounter) -> bool {
		frame_counter.wrapping_frame_index.0 % self.every_n_frames == 0
	}
}

//////////

#[derive(Copy, Clone)]
pub struct FrameCounter {
	wrapping_frame_index: Wrapping<FrameIndex>
}

impl FrameCounter {
	pub const fn new() -> Self {
		Self {wrapping_frame_index: Wrapping(0)}
	}

	pub fn tick(&mut self) {
		self.wrapping_frame_index += 1;
	}
}

//////////

#[derive(Copy, Clone)]
pub struct UpdateRateCreator {
	fps: Fps
}

impl UpdateRateCreator {
	pub const fn new(fps: Fps) -> Self {
		Self {fps}
	}

	pub fn new_instance(self, num_seconds_between_updates: Seconds) -> UpdateRate {
		UpdateRate::new(num_seconds_between_updates, self.fps)
	}
}
