use std::borrow::Cow;

use chrono::Duration;
use sdl2::{render::BlendMode, ttf::{FontStyle, Hinting}};

use crate::{
	texture::{FontInfo, TextureCreationInfo, TexturePool},
	spinitron::{model::SpinitronModelName, state::SpinitronState},

	utility_types::{
		json_utils,
		vec2f::Vec2f,
		dynamic_optional::DynamicOptional,
		generic_result::{GenericResult, MaybeError},
		update_rate::{UpdateRate, UpdateRateCreator}
	},

	window_tree::{
		ColorSDL,
		Window,
		WindowContents,
		PossibleSharedWindowStateUpdater
	},

	dashboard_defs::{
		error::make_error_window,
		credit::make_credit_window,
		weather::make_weather_window,
		shared_window_state::SharedWindowState,
		twilio::{make_twilio_window, TwilioState},
		surprise::{make_surprise_window, SurpriseCreationInfo},
		clock::{ClockHandConfig, ClockHandConfigs, ClockHands},
		spinitron::{make_spinitron_windows, SpinitronModelWindowInfo, SpinitronModelWindowsInfo}
	}
};

////////// TODO: maybe split `make_dashboard` into some smaller sub-functions

/* TODO:
- Rename all `Possible` types to `Maybe`s (incl. the associated variable names) (and all `inner-prefixed` vars too)
- Make plain texture creation less verbose through a wrapper function
*/

#[derive(serde::Deserialize)]
struct ApiKeys {
	spinitron: String,
	openweathermap: String,
	twilio_account_sid: String,
	twilio_auth_token: String
}

//////////

// This returns a top-level window, shared window state, and a shared window state updater
pub fn make_dashboard(
	texture_pool: &mut TexturePool,
	update_rate_creator: UpdateRateCreator)
	-> GenericResult<(Window, DynamicOptional, PossibleSharedWindowStateUpdater)> {

	////////// Defining some shared global variables

	const FONT_INFO: FontInfo = FontInfo {
		path: "assets/unifont/unifont-15.1.05.otf",
		unusual_chars_fallback_path: "assets/unifont/unifont_upper-15.1.05.otf",

		/* Providing this function instead of the variant below since
		`font.find_glyph` is buggy for the Rust sdl2::ttf bindings */
		font_has_char: |_, c| c as u32 <= 65535,
		// font_has_char: |font, c| font.find_glyph(c).is_some(),

		style: FontStyle::NORMAL,
		hinting: Hinting::Normal,
		maybe_outline_width: None
	};

	let top_bar_window_size_y = 0.1;
	let main_windows_gap_size = 0.01;

	let theme_color_1 = ColorSDL::RGB(249, 236, 210);
	let shared_update_rate = update_rate_creator.new_instance(15.0);
	let api_keys: ApiKeys = json_utils::load_from_file("assets/api_keys.json")?;

	////////// Defining the Spinitron window extents

	// Note: `tl` = top left
	let spin_tl = Vec2f::new_scalar(main_windows_gap_size);
	let spin_size = Vec2f::new_scalar(0.55);
	let spin_text_height = 0.03;
	let spin_tr = spin_tl.x() + spin_size.x();

	let persona_tl = Vec2f::new(spin_tr + main_windows_gap_size, spin_tl.y());
	let persona_size = Vec2f::new_scalar(0.1);

	let persona_text_tl = Vec2f::translate_y(&persona_tl, persona_size.y());
	let persona_text_height = 0.02;

	let show_tl = Vec2f::new(persona_tl.x() + persona_size.x() + main_windows_gap_size, spin_tl.y());
	let show_size = Vec2f::new_scalar(1.0 - show_tl.x() - main_windows_gap_size);

	let show_text_tl = Vec2f::translate(&(spin_tl + spin_size), 0.03, -0.2);
	let show_text_size = Vec2f::new(0.37, 0.05);

	// TODO: make a type for the top-left/size combo (and add useful utility functions from there)

	//////////

	let all_model_windows_info = [
		SpinitronModelWindowsInfo {
			model_name: SpinitronModelName::Spin,
			text_color: theme_color_1,

			texture_window: Some(SpinitronModelWindowInfo {
				tl: spin_tl,
				size: spin_size,
				border_color: Some(theme_color_1)
			}),

			text_window: Some(SpinitronModelWindowInfo {
				tl: Vec2f::translate_y(&spin_tl, spin_size.y()),
				size: Vec2f::new(spin_size.x(), spin_text_height),
				border_color: Some(theme_color_1)
			})
		},

		SpinitronModelWindowsInfo {
			model_name: SpinitronModelName::Playlist,
			text_color: theme_color_1,
			texture_window: None,
			text_window: None
		},

		// Putting show before persona here so that the persona text is drawn over
		SpinitronModelWindowsInfo {
			model_name: SpinitronModelName::Show,
			text_color: theme_color_1,

			texture_window: Some(SpinitronModelWindowInfo {
				tl: show_tl,
				size: show_size,
				border_color: Some(theme_color_1)
			}),

			text_window: Some(SpinitronModelWindowInfo {
				tl: show_text_tl,
				size: show_text_size,
				border_color: Some(theme_color_1)
			})
		},

		SpinitronModelWindowsInfo {
			model_name: SpinitronModelName::Persona,
			text_color: theme_color_1,

			texture_window: Some(SpinitronModelWindowInfo {
				tl: persona_tl,
				size: persona_size,
				border_color: Some(theme_color_1)
			}),

			text_window: Some(SpinitronModelWindowInfo {
				tl: persona_text_tl,
				size: Vec2f::new(persona_size.x(), persona_text_height),
				border_color: Some(theme_color_1)
			})
		}
	];

	// The Spinitron windows update at the same rate as the shared update rate
	let spinitron_windows = make_spinitron_windows(
		&all_model_windows_info, shared_update_rate
	);

	////////// Making a Twilio window

	let twilio_state = TwilioState::new(
		&api_keys.twilio_account_sid,
		&api_keys.twilio_auth_token,
		6,
		Duration::days(5),
		false
	);

	let twilio_window = make_twilio_window(
		&twilio_state,

		// This is how often the history windows check for new messages (this is low so that it'll be fast in the beginning)
		update_rate_creator.new_instance(0.25),

		Vec2f::new(0.58, 0.45), Vec2f::new(0.4, 0.27),

		0.025,
		WindowContents::Color(ColorSDL::RGB(0, 200, 0)),

		Vec2f::new(0.1, 0.45),
		theme_color_1, theme_color_1,

		WindowContents::make_texture_contents("assets/text_bubble.png", texture_pool)?
	);

	////////// Making an error window

	let error_window = make_error_window(
		Vec2f::new(0.0, 0.95),
		Vec2f::new(0.15, 0.05),
		update_rate_creator.new_instance(2.0),
		WindowContents::Color(ColorSDL::RGBA(255, 0, 0, 190)),
		ColorSDL::GREEN
	);

	////////// Making a credit window

	let credit_window = make_credit_window(
		Vec2f::new(0.85, 0.97),
		Vec2f::new(0.15, 0.03),
		ColorSDL::RED,
		ColorSDL::RGB(210, 180, 140),
		"By: Caspian Ahlberg"
	);

	////////// Making a clock window

	let clock_size_x = top_bar_window_size_y;
	let clock_tl = Vec2f::new(1.0 - clock_size_x, 0.0);
	let clock_size = Vec2f::new(clock_size_x, 1.0);

	let (clock_hands, clock_window) = ClockHands::new_with_window(
		UpdateRate::ONCE_PER_FRAME,
		clock_tl,
		clock_size,

		ClockHandConfigs {
			milliseconds: ClockHandConfig::new(0.01, 0.2, 0.5, ColorSDL::RGBA(255, 0, 0, 100)), // Milliseconds
			seconds: ClockHandConfig::new(0.01, 0.02, 0.48, ColorSDL::WHITE), // Seconds
			minutes: ClockHandConfig::new(0.01, 0.02, 0.35, ColorSDL::YELLOW), // Minutes
			hours: ClockHandConfig::new(0.01, 0.02, 0.2, ColorSDL::BLACK) // Hours
		},

		"assets/watch_dial.png",
		texture_pool
	)?;

	////////// Making a weather window

	let weather_window = make_weather_window(
		Vec2f::ZERO,
		Vec2f::new(0.4, 0.3),
		update_rate_creator,
		&api_keys.openweathermap,
		"Brunswick",
		"ME",
		"US"
	);

	////////// Making some static texture windows

	// Texture path, top left, size (TODO: make animated textures possible)
	let main_static_texture_info = [
		("assets/dashboard_bookshelf.png", Vec2f::ZERO, Vec2f::ONE, false),
		("assets/logo.png", Vec2f::new(0.6, 0.75), Vec2f::new(0.1, 0.05), false),
		("assets/soup.png", Vec2f::new(0.45, 0.72), Vec2f::new(0.06666666, 0.1), false),
		("assets/ness.bmp", Vec2f::new(0.28, 0.73), Vec2f::new_scalar(0.08), false)
	];

	let foreground_static_texture_info = [
		("assets/dashboard_foreground.png", Vec2f::ZERO, Vec2f::ONE, true)
	];

	let background_static_texture_info = [
		// "assets/dashboard_background.png"
	];

	let add_static_texture_set =
		|set: &mut Vec<Window>, all_info: &[(&'static str, Vec2f, Vec2f, bool)], texture_pool: &mut TexturePool| {

		set.extend(all_info.iter().map(|&(path, tl, size, skip_ar_correction)| {
			let mut window = Window::new(
				None,
				DynamicOptional::NONE,
				WindowContents::make_texture_contents(path, texture_pool).unwrap(),
				None,
				tl,
				size,
				None
			);

			window.set_aspect_ratio_correction_skipping(skip_ar_correction);
			window
		}))
	};

	let mut all_main_windows = vec![twilio_window, error_window, credit_window];
	all_main_windows.extend(spinitron_windows);
	add_static_texture_set(&mut all_main_windows, &main_static_texture_info, texture_pool);

	////////// Making all of the main windows

	let main_window_tl_y = main_windows_gap_size + top_bar_window_size_y + main_windows_gap_size;
	let main_window_size_y = 1.0 - main_window_tl_y - main_windows_gap_size;
	let x_width_from_main_window_gap_size = 1.0 - main_windows_gap_size * 2.0;

	let top_bar_tl = Vec2f::new_scalar(main_windows_gap_size);

	let top_bar_window = Window::new(
		None,
		DynamicOptional::NONE,
		WindowContents::Color(ColorSDL::RGB(128, 0, 32)),
		None,
		top_bar_tl,
		Vec2f::new(x_width_from_main_window_gap_size, top_bar_window_size_y),
		Some(vec![clock_window, weather_window])
	);

	let mut main_window = Window::new(
		None,
		DynamicOptional::NONE,

		WindowContents::Many(
			background_static_texture_info.into_iter().map(|path|
				WindowContents::make_texture_contents(path, texture_pool)
			).collect::<GenericResult<_>>()?
		),

		Some(theme_color_1),
		Vec2f::new(main_windows_gap_size, main_window_tl_y),
		Vec2f::new(x_width_from_main_window_gap_size, main_window_size_y),
		Some(all_main_windows)
	);

	main_window.set_aspect_ratio_correction_skipping(true);

	////////// Making a surprise window

	let surprise_window = make_surprise_window(Vec2f::ZERO, Vec2f::ONE,
		&[
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
				chance_of_appearing_when_updating: 0.000003,

				local_hours_24_start: 0,
				local_hours_24_end: 5,

				flicker_window: true
			},

			SurpriseCreationInfo {
				texture_path: "assets/horrible.webp",
				texture_blend_mode: BlendMode::Mul,

				update_rate: Duration::milliseconds(100),
				num_update_steps_to_appear_for: 9,
				chance_of_appearing_when_updating: 0.0, // This one can only be triggered artificially

				local_hours_24_start: 0,
				local_hours_24_end: 23,

				flicker_window: true
			},
		],

		update_rate_creator,
		texture_pool
	)?;

	////////// Making the highest-level window

	let mut all_windows = vec![top_bar_window, main_window];
	add_static_texture_set(&mut all_windows, &foreground_static_texture_info, texture_pool);
	all_windows.push(surprise_window);

	let all_windows_window = Window::new(
		None,
		DynamicOptional::NONE,
		WindowContents::Nothing,
		None,
		Vec2f::ZERO,
		Vec2f::ONE,
		Some(all_windows)
	);

	////////// Defining the shared state

	// TODO: make it possible to get different variants of this texture (randomly chosen)
	const FALLBACK_TEXTURE_CREATION_INFO: TextureCreationInfo<'static> =
		TextureCreationInfo::Path(Cow::Borrowed("assets/no_texture_available.png"));

	let initial_spin_window_size_guess = (1000, 1000);
	let spin_expiry_duration = Duration::minutes(20);

	let spinitron_state = SpinitronState::new(
		(&api_keys.spinitron, spin_expiry_duration,
		&FALLBACK_TEXTURE_CREATION_INFO, initial_spin_window_size_guess)
	)?;

	let boxed_shared_state = DynamicOptional::new(
		SharedWindowState {
			clock_hands,
			spinitron_state,
			twilio_state,
			font_info: &FONT_INFO,
			fallback_texture_creation_info: &FALLBACK_TEXTURE_CREATION_INFO,
			curr_dashboard_error: None,
			rand_generator: rand::thread_rng()
		}
	);

	fn shared_window_state_updater(state: &mut DynamicOptional, texture_pool: &mut TexturePool) -> MaybeError {
		let state = state.get_mut::<SharedWindowState>();

		let mut error = None;

		// More continual updaters can be added here
		let success_states_and_names = [
			(state.spinitron_state.update()?, "Spinitron"),
			(state.twilio_state.update(texture_pool)?, "Twilio (messaging)")
		];

		for (succeeded, name) in success_states_and_names {
			if !succeeded {
				if let Some(already_error) = &mut error {
					*already_error += ", and ";
					*already_error += name;
				}
				else {
					error = Some(format!("Internal dashboard error from {name}"))
				}
			}
		}

		if let Some(inner_error) = &mut error {
			*inner_error += "! ";
		}

		state.curr_dashboard_error = error;

		Ok(())
	}

	//////////

	Ok((
		all_windows_window,
		boxed_shared_state,
		Some((shared_window_state_updater, shared_update_rate))
	))
}
