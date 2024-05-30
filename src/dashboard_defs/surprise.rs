use std::{
	rc::Rc,
	borrow::Cow,
	cell::RefCell,
	sync::{Arc, atomic::{AtomicBool, Ordering}}
};

use chrono::Timelike;

use crate::{
	window_tree::{
		Window,
		WindowContents,
		WindowUpdaterParams
	},

	utility_types::{
		dynamic_optional::DynamicOptional,
		vec2f::{Vec2f, assert_in_unit_interval},
		update_rate::{UpdateRateCreator, Seconds},
		generic_result::{GenericResult, MaybeError}
	},

	texture::{TexturePool, TextureCreationInfo},
	dashboard_defs::shared_window_state::SharedWindowState
};

/* Note: some surprises may take somewhat long to be
triggered if their update rates are relatively infrequent. */

type NumAppearanceSteps = u16;
type SurpriseAppearanceChance = f64; // 0 to 1

pub struct SurpriseCreationInfo<'a> {
	pub texture_path: &'a str,
	pub texture_blend_mode: sdl2::render::BlendMode,

	pub update_rate: chrono::Duration,
	pub num_update_steps_to_appear_for: NumAppearanceSteps,
	pub chance_of_appearing_when_updating: SurpriseAppearanceChance,

	pub local_hours_24_start: u8,
	pub local_hours_24_end: u8,

	pub flicker_window: bool
}

//////////

/* TODO: use local sockets (domain sockets under the hood) to send a file path, instead of
an index (if not, send some part of a file path, or perhaps the inode, with signals) */

pub fn make_surprise_window(
	top_left: Vec2f, size: Vec2f,
	surprise_creation_info: &[SurpriseCreationInfo],
	update_rate_creator: UpdateRateCreator,
	texture_pool: &mut TexturePool) -> GenericResult<Window> {

	////////// Some internally used types

	struct SharedSurpriseInfo {
		num_surprises: usize,
		curr_signaled_index: usize,
		the_trigger_index_was_incremented: Arc<AtomicBool>,
		one_was_artificially_triggered: Arc<AtomicBool>
	}

	struct SurpriseInfo {
		index: usize,

		num_update_steps_to_appear_for: NumAppearanceSteps,
		chance_of_appearing_when_updating: SurpriseAppearanceChance, // 0 to 1
		curr_num_steps_when_appeared: Option<NumAppearanceSteps>, // if this is `None`, we are not in the appearance period

		local_hours_24_start: u8,
		local_hours_24_end: u8,
		flicker_window: bool,

		// This is wrapped in a `Rc<RefCell<_>>` because the info is shared and mutable
		shared_info: Rc<RefCell<SharedSurpriseInfo>>
	}

	////////// Some utility functions

	fn appearance_was_randomly_triggered(surprise_info: &SurpriseInfo, rand_generator: &mut rand::rngs::ThreadRng) -> bool {
		let local_hour = chrono::Local::now().hour();

		let in_acceptable_hour_range =
			local_hour >= surprise_info.local_hours_24_start.into()
			&& local_hour <= surprise_info.local_hours_24_end.into();

		use rand::Rng; // TODO: can I use the system's rand generator instead? Less dependencies that way...
		let rand_num = rand_generator.gen::<SurpriseAppearanceChance>();

		in_acceptable_hour_range && rand_num < surprise_info.chance_of_appearing_when_updating
	}

	// An appearance is considered artificially triggered if it was activated by the `trigger_surprise.bash` script
	fn appearance_was_artificially_triggered(s: &RefCell<SharedSurpriseInfo>, index: usize) -> bool {
		let check_and_clear_atomic_bool = |b: &Arc<AtomicBool>| b.swap(false, Ordering::Relaxed);

		let mut sb = s.borrow();

		if check_and_clear_atomic_bool(&sb.the_trigger_index_was_incremented) {
			let num_surprises = sb.num_surprises;
			drop(sb); // Dropping it manually because otherwise, the `borrow_mut` below will panic

			let curr_index = &mut s.borrow_mut().curr_signaled_index;
			*curr_index += 1;

			if *curr_index >= num_surprises {
				log::warn!(
					"While incrementing the surprise index, you exceeded the maximum! Your index will \
					be interpreted as `index % num_surprises` (there are {} total surprises).",
					num_surprises
				);

				*curr_index = 0;
			}
		}

		sb = s.borrow();
		sb.curr_signaled_index == index && check_and_clear_atomic_bool(&sb.one_was_artificially_triggered)
	}

	fn updater_fn(params: WindowUpdaterParams) -> MaybeError {
		let surprise_info = params.window.get_state_mut::<SurpriseInfo>();
		let rand_generator = &mut params.shared_window_state.get_mut::<SharedWindowState>().rand_generator;

		let trigger_appearance_by_chance = appearance_was_randomly_triggered(surprise_info, rand_generator);
		let trigger_appearance_artificially = appearance_was_artificially_triggered(&surprise_info.shared_info, surprise_info.index);
		let not_currently_active = surprise_info.curr_num_steps_when_appeared.is_none();

		if (trigger_appearance_by_chance || trigger_appearance_artificially) && not_currently_active {
			log::info!("Trigger surprise with index {}!", surprise_info.index);
			surprise_info.curr_num_steps_when_appeared = Some(0);
		}

		if let Some(num_steps_when_appeared) = &mut surprise_info.curr_num_steps_when_appeared {
			*num_steps_when_appeared += 1;

			let stop_showing = *num_steps_when_appeared == surprise_info.num_update_steps_to_appear_for + 1;

			let should_skip_drawing = if stop_showing {
				surprise_info.shared_info.borrow_mut().curr_signaled_index = 0; // Reset the index back to 0 for next time
				surprise_info.curr_num_steps_when_appeared = None;
				true
			}
			else if surprise_info.flicker_window {
				!params.window.drawing_is_skipped()
			}
			else {
				false
			};

			params.window.set_draw_skipping(should_skip_drawing)
		}

		Ok(())
	}

	////////// Setting up the shared surprise info that can be triggered via signals

	let make_signal_bool = |signal_value| -> GenericResult<Arc<AtomicBool>> {
		let the_signal_bool = Arc::new(AtomicBool::new(false));
		signal_hook::flag::register(signal_value, the_signal_bool.clone())?;
		Ok(the_signal_bool)
	};

	use signal_hook::consts::signal;

	let shared_surprise_info = Rc::new(RefCell::new(SharedSurpriseInfo {
		num_surprises: surprise_creation_info.len(),
		curr_signaled_index: 0,
		the_trigger_index_was_incremented: make_signal_bool(signal::SIGUSR1)?,
		one_was_artificially_triggered: make_signal_bool(signal::SIGUSR2)?
	}));

	////////// Making the surprise windows

	let surprise_windows = surprise_creation_info.iter().enumerate().map(
		|(index, creation_info)| {
			assert_in_unit_interval(creation_info.chance_of_appearing_when_updating as f32);

			/* The lower bound checks that it actually appears, and the upper
			bound checks that the ` + 1` in the updater does not overflow */
			assert!(creation_info.num_update_steps_to_appear_for > 0
				&& creation_info.num_update_steps_to_appear_for < NumAppearanceSteps::MAX);

			const MAX_HOUR_INDEX_FOR_DAY: u8 = 23;
			assert!(creation_info.local_hours_24_start <= MAX_HOUR_INDEX_FOR_DAY);
			assert!(creation_info.local_hours_24_end <= MAX_HOUR_INDEX_FOR_DAY);

			//////////

			let update_rate_secs =
				creation_info.update_rate.num_seconds() as Seconds +
				creation_info.update_rate.subsec_nanos() as Seconds / 1_000_000_000.0;

			log::info!(
				"Surprise '{}' will occur approximately every {:.3} seconds (from {}:00 to {}:00), and then {} for {:.3} seconds{}.",
				creation_info.texture_path,
				update_rate_secs / creation_info.chance_of_appearing_when_updating as Seconds,

				creation_info.local_hours_24_start,
				creation_info.local_hours_24_end,

				if creation_info.flicker_window {"flicker"} else {"persist"},
				update_rate_secs * creation_info.num_update_steps_to_appear_for as Seconds,

				if creation_info.flicker_window {Cow::Owned(format!(", with {}-second off-cycles", update_rate_secs))}
				else {Cow::Borrowed("")}
			);

			//////////

			let update_rate = update_rate_creator.new_instance(update_rate_secs);
			let texture_creation_info = TextureCreationInfo::Path(Cow::Borrowed(creation_info.texture_path));

			let texture = texture_pool.make_texture(&texture_creation_info)?;
			texture_pool.set_blend_mode_for(&texture, creation_info.texture_blend_mode);

			let mut window = Window::new(
				Some((updater_fn, update_rate)),

				DynamicOptional::new(SurpriseInfo {
					index,

					num_update_steps_to_appear_for: creation_info.num_update_steps_to_appear_for,
					chance_of_appearing_when_updating: creation_info.chance_of_appearing_when_updating,
					curr_num_steps_when_appeared: None,

					local_hours_24_start: creation_info.local_hours_24_start,
					local_hours_24_end: creation_info.local_hours_24_end,
					flicker_window: creation_info.flicker_window,

					shared_info: shared_surprise_info.clone()
				}),

				WindowContents::Texture(texture),
				None,
				Vec2f::ZERO,
				Vec2f::ONE,
				None
			);

			window.set_draw_skipping(true);
			window.set_aspect_ratio_correction_skipping(true);
			Ok(window)
		}
	).collect::<GenericResult<_>>()?;

	Ok(Window::new(
		None,
		DynamicOptional::NONE,
		WindowContents::Nothing,
		None,
		top_left,
		size,
		Some(surprise_windows)
	))
}
