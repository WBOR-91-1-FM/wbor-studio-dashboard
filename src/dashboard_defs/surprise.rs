use std::{
	rc::Rc,
	borrow::Cow,
	cell::RefCell,
	collections::HashSet
};

use crate::{
	window_tree::{
		Window,
		WindowContents,
		WindowUpdaterParams
	},

	utils::{
		ipc::*,
		time::*,
		generic_result::*,
		dynamic_optional::DynamicOptional,
		vec2f::{Vec2f, assert_in_unit_interval},
		update_rate::{Seconds, UpdateRateCreator}
	},

	texture::pool::{TexturePool, TextureCreationInfo},
	dashboard_defs::shared_window_state::SharedWindowState

};

/* Note: some surprises may take somewhat long to be
triggered if their update rates are relatively infrequent.
TODO: make a separate updater for just getting the artificial
triggering going (this will be the socket-polling updater). */

// TODO: display DJ tips as surprises

type NumAppearanceSteps = u16;
type SurpriseAppearanceChance = f64; // 0 to 1

pub struct SurpriseCreationInfo {
	pub texture_path: &'static str,
	pub texture_blend_mode: sdl2::render::BlendMode,

	pub update_rate: Duration,
	pub num_update_steps_to_appear_for: NumAppearanceSteps,
	pub chance_of_appearing_when_updating: SurpriseAppearanceChance,

	pub local_hours_24_start: u8,
	pub local_hours_24_end: u8,

	pub flicker_window: bool
}

//////////

pub async fn make_surprise_window(
	top_left: Vec2f, size: Vec2f,
	artificial_triggering_socket_path: &str,
	surprise_creation_info: &[SurpriseCreationInfo],
	update_rate_creator: UpdateRateCreator,
	texture_pool: &mut TexturePool<'_>) -> GenericResult<Window> {

	////////// Some internally used types

	type SurprisePath = &'static str;

	struct SharedSurpriseInfo {
		surprise_path_set: HashSet<SurprisePath>,
		queued_surprise_paths: Vec<SurprisePath>, // A multiset would be better here...
		surprise_stream_listener: IpcSocketListener
	}

	struct SurpriseInfo {
		path: SurprisePath,

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
		let local_hour = get_local_time().hour();

		let in_acceptable_hour_range =
			local_hour >= surprise_info.local_hours_24_start.into()
			&& local_hour <= surprise_info.local_hours_24_end.into();

		use rand::Rng;
		let rand_num = rand_generator.gen::<SurpriseAppearanceChance>();

		in_acceptable_hour_range && rand_num < surprise_info.chance_of_appearing_when_updating
	}

	////////// The core updater function that runs once every N milliseconds for each surprise

	fn updater_fn(params: WindowUpdaterParams) -> MaybeError {
		let surprise_info = params.window.get_state_mut::<SurpriseInfo>();
		let rand_generator = &mut params.shared_window_state.get_mut::<SharedWindowState>().rand_generator;

		let not_currently_active = surprise_info.curr_num_steps_when_appeared.is_none();

		// The braces are here to keep the borrow checker happy
		let trigger_appearance_artificially = not_currently_active && {
			let mut shared_info = surprise_info.shared_info.borrow_mut();

			if let Some(path) = try_listening_to_ipc_socket(&mut shared_info.surprise_stream_listener) {
				if let Some(&matching_path) = shared_info.surprise_path_set.get(path.as_str()) {
					shared_info.queued_surprise_paths.push(matching_path);
				}
				else {
					log::warn!("Tried to trigger a surprise with a path of '{path}', but no surprise has that path!");
				}
			}

			// This runs if the path of the current surprise (per this updater call) is in the queue
			if let Some(index_in_queue) = shared_info.queued_surprise_paths.iter().position(|s| s == &surprise_info.path) {
				shared_info.queued_surprise_paths.remove(index_in_queue);
				true
			}
			else {
				false
			}
		};

		let trigger_appearance_by_chance = appearance_was_randomly_triggered(surprise_info, rand_generator);

		if (trigger_appearance_by_chance || trigger_appearance_artificially) && not_currently_active {
			log::info!("Trigger surprise with path '{}'!", surprise_info.path);
			surprise_info.curr_num_steps_when_appeared = Some(0);
		}

		if let Some(num_steps_when_appeared) = &mut surprise_info.curr_num_steps_when_appeared {
			*num_steps_when_appeared += 1;

			let stop_showing = *num_steps_when_appeared == surprise_info.num_update_steps_to_appear_for + 1;

			let should_skip_drawing = if stop_showing {
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

	////////// First, checking for duplicate paths, and failing if this is the case

	let make_path_iterator = || surprise_creation_info.iter().map(|info| info.texture_path);

	let surprise_paths: Vec<SurprisePath> = make_path_iterator().collect();
	let surprise_path_set: HashSet<SurprisePath> = make_path_iterator().collect();

	if surprise_path_set.len() != surprise_creation_info.len() {
		return error_msg!("There are duplicate paths in the set of surprises");
	}

	////////// Setting up the shared surprise info that can be triggered via IPC

	let (all_creation_info, surprise_stream_listener) = tokio::try_join!(
		TextureCreationInfo::from_paths_async(make_path_iterator()),
		make_ipc_socket_listener(artificial_triggering_socket_path)
	)?;

	let shared_surprise_info = Rc::new(RefCell::new(SharedSurpriseInfo {
		surprise_path_set,
		queued_surprise_paths: Vec::new(),
		surprise_stream_listener
	}));

	////////// Making the surprise windows

	let surprise_windows = surprise_creation_info.iter().enumerate().zip(all_creation_info).map(
		|((index, creation_info), texture_creation_info)| {
			assert_in_unit_interval(creation_info.chance_of_appearing_when_updating);

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

				if creation_info.flicker_window {Cow::Owned(format!(", with {update_rate_secs}-second off-cycles"))}
				else {Cow::Borrowed("")}
			);

			//////////

			let texture = texture_pool.make_texture(&texture_creation_info)?;
			let update_rate = update_rate_creator.new_instance(update_rate_secs);

			texture_pool.set_blend_mode_for(&texture, creation_info.texture_blend_mode);

			let mut window = Window::new(
				vec![(updater_fn, update_rate)],

				DynamicOptional::new(SurpriseInfo {
					path: surprise_paths[index],

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
				vec![]
			);

			window.set_draw_skipping(true);
			window.set_aspect_ratio_correction_skipping(true);
			Ok(window)
		}
	).collect::<GenericResult<Vec<_>>>()?;

	Ok(Window::new(
		vec![],
		DynamicOptional::NONE,
		WindowContents::Nothing,
		None,
		top_left,
		size,
		surprise_windows
	))
}
