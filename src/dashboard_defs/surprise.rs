use std::{
	rc::Rc,
	borrow::Cow,
	cell::RefCell,
	collections::HashSet,
	io::{BufRead, BufReader}
};

use chrono::Timelike;

use interprocess::local_socket::{
	ToFsName,
	GenericFilePath,
	ListenerOptions,
	traits::Listener,
	ListenerNonblockingMode,
	prelude::LocalSocketListener
};

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

// TODO: display DJ tips as surprises

//////////

pub fn make_surprise_window(
	top_left: Vec2f, size: Vec2f,
	artificial_triggering_socket_path: &str,
	surprise_creation_info: &[SurpriseCreationInfo],
	update_rate_creator: UpdateRateCreator,
	texture_pool: &mut TexturePool) -> GenericResult<Window> {

	////////// Some internally used types

	type SurprisePath=Rc<String>;

	struct SharedSurpriseInfo {
		surprise_path_set: HashSet<SurprisePath>,
		queued_surprise_paths: Vec<SurprisePath>, // A multiset would be better here...
		surprise_stream_listener: LocalSocketListener,
		surprise_stream_path_buffer: String
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
		let local_hour = chrono::Local::now().hour();

		let in_acceptable_hour_range =
			local_hour >= surprise_info.local_hours_24_start.into()
			&& local_hour <= surprise_info.local_hours_24_end.into();

		use rand::Rng; // TODO: can I use the system's rand generator instead? Less dependencies that way...
		let rand_num = rand_generator.gen::<SurpriseAppearanceChance>();

		in_acceptable_hour_range && rand_num < surprise_info.chance_of_appearing_when_updating
	}

	////////// The core updater function that runs once every N milliseconds for each surprise

	// TODO: make a separate updater for just getting the artificial triggering going (this will be the socket-polling updater)

	fn updater_fn(params: WindowUpdaterParams) -> MaybeError {
		let surprise_info = params.window.get_state_mut::<SurpriseInfo>();
		let rand_generator = &mut params.shared_window_state.get_mut::<SharedWindowState>().rand_generator;

		let not_currently_active = surprise_info.curr_num_steps_when_appeared.is_none();

		// The braces are here to keep the borrow checker happy
		let trigger_appearance_artificially = not_currently_active && {
			let mut shared_info = surprise_info.shared_info.borrow_mut();

			// TODO: include some error handling here
			if let Some(Ok(stream)) = shared_info.surprise_stream_listener.next() {
				let mut reader = BufReader::new(stream);
				reader.read_line(&mut shared_info.surprise_stream_path_buffer)?;

				if let Some(matching_path) = shared_info.surprise_path_set.get(&shared_info.surprise_stream_path_buffer) {
					let rc_cloned_matching_path = matching_path.clone();
					shared_info.queued_surprise_paths.push(rc_cloned_matching_path);
				}
				else {
					log::warn!("Tried to trigger a surprise with a path of '{}', but no surprise has that path!",
						shared_info.surprise_stream_path_buffer);
				}

				shared_info.surprise_stream_path_buffer.clear();
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

	let surprise_paths: Vec<SurprisePath> = surprise_creation_info.iter().map(|info| info.texture_path.to_string().into()).collect();
	let surprise_path_set: HashSet<SurprisePath> = surprise_paths.iter().map(Rc::clone).collect();

	if surprise_path_set.len() != surprise_creation_info.len() {
		return Err("There are duplicate paths in the set of surprises".into());
	}

	////////// Setting up the shared surprise info that can be triggered via signals

	const SURPRISE_STREAM_PATH_BUFFER_INITIAL_SIZE: usize = 64;

	let options = ListenerOptions::new().name(artificial_triggering_socket_path.to_fs_name::<GenericFilePath>()?);

	let surprise_stream_listener = options.create_sync()?;
	surprise_stream_listener.set_nonblocking(ListenerNonblockingMode::Both)?;

	let shared_surprise_info = Rc::new(RefCell::new(SharedSurpriseInfo {
		surprise_path_set,
		queued_surprise_paths: Vec::new(),
		surprise_stream_listener,
		surprise_stream_path_buffer: String::with_capacity(SURPRISE_STREAM_PATH_BUFFER_INITIAL_SIZE)
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
					path: surprise_paths[index].clone(),

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
