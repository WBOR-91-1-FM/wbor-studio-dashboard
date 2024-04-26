use std::borrow::Cow;

use chrono::Timelike;

use crate::{
	texture::{TexturePool, TextureCreationInfo},

	utility_types::{
		generic_result::{GenericResult, MaybeError},
		dynamic_optional::DynamicOptional,
		vec2f::{Vec2f, assert_in_unit_interval},
		update_rate::{UpdateRateCreator, Seconds}
	},

	window_tree::{
		Window,
		WindowContents,
		WindowUpdaterParams
	},

	dashboard_defs::shared_window_state::SharedWindowState
};

type NumAppearanceSteps = u16;
type SurpriseAppearanceChance = f64; // 0 to 1

pub struct SurpriseCreationInfo<'a> {
	pub texture_path: &'a str,
	pub texture_blend_mode: sdl2::render::BlendMode,

	pub update_rate: chrono::Duration,
	pub chance_of_appearing_when_updating: SurpriseAppearanceChance,
	pub num_update_steps_to_appear_for: NumAppearanceSteps,

	pub local_hours_24_start: u8,
	pub local_hours_24_end: u8,

	pub flicker_window: bool
}

pub fn make_surprise_window(
	top_left: Vec2f, size: Vec2f,
	surprise_creation_info: &[SurpriseCreationInfo],
	update_rate_creator: UpdateRateCreator,
	texture_pool: &mut TexturePool) -> GenericResult<Window> {

	struct SurpriseInfo {
		chance_of_appearing_when_updating: SurpriseAppearanceChance, // 0 to 1
		num_update_steps_to_appear_for: NumAppearanceSteps,
		local_hours_24_start: u8,
		local_hours_24_end: u8,
		flicker_window: bool,
		curr_num_steps_when_appeared: Option<NumAppearanceSteps> // if this is `None`, we are not in the appearance period
	}

	fn updater_fn(params: WindowUpdaterParams) -> MaybeError {
		let inner_shared_state = params.shared_window_state.get_mut::<SharedWindowState>();
		let surprise_info = params.window.get_state_mut::<SurpriseInfo>();

		//////////

		use rand::Rng; // TODO: can I use the system's rand generator instead? Less dependencies that way...
		let rand_num = inner_shared_state.rand_generator.gen::<SurpriseAppearanceChance>();

		let local_hour = chrono::Local::now().hour();

		let in_acceptable_hour_range =
			local_hour >= surprise_info.local_hours_24_start.into()
			&& local_hour <= surprise_info.local_hours_24_end.into();

		let trigger_appearance =
			in_acceptable_hour_range
			&& rand_num < surprise_info.chance_of_appearing_when_updating
			&& surprise_info.curr_num_steps_when_appeared.is_none();

		//////////

		if trigger_appearance {
			surprise_info.curr_num_steps_when_appeared = Some(0);
			log::info!("Trigger surprise!");
		}

		if let Some(curr_num_steps) = &mut surprise_info.curr_num_steps_when_appeared {
			*curr_num_steps += 1;

			let stop_showing = *curr_num_steps == surprise_info.num_update_steps_to_appear_for + 1;

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

	let surprise_windows = surprise_creation_info.iter().map(
		|creation_info| {
			assert_in_unit_interval(creation_info.chance_of_appearing_when_updating as f32);

			/* The lower bound checks that it actually appears, and the upper
			bound checks that the ` + 1` in the updater does not overflow */
			assert!(creation_info.num_update_steps_to_appear_for > 0
				&& creation_info.num_update_steps_to_appear_for < NumAppearanceSteps::MAX);

			assert!(creation_info.local_hours_24_start <= 23);
			assert!(creation_info.local_hours_24_end <= 23);

			//////////

			let update_rate_secs =
				creation_info.update_rate.num_seconds() as Seconds +
				creation_info.update_rate.subsec_nanos() as Seconds / 1_000_000_000.0;

			log::info!(
				"Surprise with path '{}' will occur approximately once every {:.3} seconds, and then {} for {:.3} seconds{}",
				creation_info.texture_path,
				update_rate_secs / creation_info.chance_of_appearing_when_updating as Seconds,

				if creation_info.flicker_window {"flicker"} else {"persist"},
				update_rate_secs * creation_info.num_update_steps_to_appear_for as Seconds,

				if creation_info.flicker_window {Cow::Owned(format!(", with off-cycles lasting for {} seconds", update_rate_secs))}
				else {Cow::Borrowed("")}
			);

			//////////

			let update_rate = update_rate_creator.new_instance(update_rate_secs);
			let texture_creation_info = TextureCreationInfo::Path(Cow::Borrowed(creation_info.texture_path));

			let texture = texture_pool.make_texture(&texture_creation_info)?;
			texture_pool.set_blend_mode_for(&texture, creation_info.texture_blend_mode);

			// TODO: when initializing textures, perhaps set a default blend mode of `None`, for the sake of speed (adjust this in other spots later though)

			let mut window = Window::new(
				Some((updater_fn, update_rate)),

				DynamicOptional::new(SurpriseInfo {
					chance_of_appearing_when_updating: creation_info.chance_of_appearing_when_updating,
					num_update_steps_to_appear_for: creation_info.num_update_steps_to_appear_for,
					local_hours_24_start: creation_info.local_hours_24_start,
					local_hours_24_end: creation_info.local_hours_24_end,
					flicker_window: creation_info.flicker_window,
					curr_num_steps_when_appeared: None
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
