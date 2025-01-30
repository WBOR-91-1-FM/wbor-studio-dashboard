use sdl2::{render::BlendMode, ttf::{FontStyle, Hinting}};

use crate::{
	themes::shared_utils::*,
	spinitron::{model::SpinitronModelName, state::SpinitronState},

	texture::{
		text::FontInfo,
		pool::{TextureCreationInfo, TexturePool, RemakeTransitionInfo}
	},

	utility_types::{
		file_utils,
		vec2f::Vec2f,
		time::Duration,
		generic_result::*,
		dynamic_optional::DynamicOptional,
		update_rate::{UpdateRate, UpdateRateCreator}
	},

	window_tree::{
		ColorSDL,
		Window,
		WindowContents
	},

	dashboard_defs::{
		easing_fns,
		credit::make_credit_window,
		weather::make_weather_window,
		error::{make_error_window, ErrorState},
		shared_window_state::SharedWindowState,
		twilio::{make_twilio_window, TwilioState},
		surprise::{make_surprise_window, SurpriseCreationInfo},
		clock::{ClockHandConfig, ClockHandConfigs, ClockHands},
		streaming_server_status::make_streaming_server_status_window,
		spinitron::{make_spinitron_windows, SpinitronModelWindowInfo, SpinitronModelWindowsInfo}
	}
};

////////// TODO: maybe split `make_dashboard` into some smaller sub-functions

/* TODO:
- Rename all `Possible` types to `Maybe`s (incl. the associated variable names) (and all `inner-prefixed` vars too)
- Make plain texture creation less verbose through a wrapper function
*/

// This returns a top-level window, and shared window state
pub async fn make_dashboard(
	texture_pool: &mut TexturePool<'_>,
	update_rate_creator: UpdateRateCreator)
	-> GenericResult<(Window, DynamicOptional)> {

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

	let main_windows_gap_size = 0.01;

	let theme_color_1 = ColorSDL::RGB(255, 133, 133);
	let shared_update_rate = update_rate_creator.new_instance(10.0);
	let api_keys: ApiKeys = file_utils::load_json_from_file("assets/api_keys.json").await?;

	////////// Defining the Spinitron window extents

	// Note: `tl` = top left
	let spin_tl = Vec2f::new_scalar(main_windows_gap_size);
	let spin_size = Vec2f::new(0.55, 0.81);
	let spin_tr = spin_tl.x() + spin_size.x();

	let spin_text_height = 0.03;
	let spin_text_tl = Vec2f::translate_y(&spin_tl, spin_size.y());
	let spin_text_size = Vec2f::new(spin_size.x(), spin_text_height);

	let persona_tl = Vec2f::new(spin_tr + main_windows_gap_size, spin_tl.y());
	let persona_size = Vec2f::new(0.2, 0.3);

	/*
	let persona_text_tl = Vec2f::translate_y(&persona_tl, 0.0);
	let persona_text_height = 0.05;
	*/

	let playlist_tl = Vec2f::new(persona_tl.x() + persona_size.x() + main_windows_gap_size, spin_tl.y());
	let playlist_size = persona_size;

	let text_scalar = Vec2f::new_scalar(0.55);
	let playlist_text_tl = Vec2f::translate(&(spin_tl + text_scalar), 0.03, -0.24);
	let playlist_text_size = Vec2f::new(0.37, 0.05);

	// TODO: make a type for the top-left/size combo (and add useful utility functions from there)

	////////// Making the Spinitron windows

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
				tl: spin_text_tl,
				size: spin_text_size,
				border_color: Some(theme_color_1)
			})
		},

		SpinitronModelWindowsInfo {
			model_name: SpinitronModelName::Playlist,
			text_color: theme_color_1,

			texture_window: Some(SpinitronModelWindowInfo {
				tl: playlist_tl,
				size: playlist_size,
				border_color: Some(theme_color_1)
			}),

			text_window: Some(SpinitronModelWindowInfo {
				tl: playlist_text_tl,
				size: playlist_text_size,
				border_color: Some(theme_color_1)
			})
		},

		// Putting show before persona here so that the persona text is drawn over (not used at the moment though)
		SpinitronModelWindowsInfo {
			model_name: SpinitronModelName::Show,
			text_color: theme_color_1,
			texture_window: None,
			text_window: None
		},

		SpinitronModelWindowsInfo {
			model_name: SpinitronModelName::Persona,
			text_color: theme_color_1,

			texture_window: Some(SpinitronModelWindowInfo {
				tl: persona_tl,
				size: persona_size,
				border_color: Some(theme_color_1)
			}),

			text_window: None

			/*
			text_window: Some(SpinitronModelWindowInfo {
				tl: persona_text_tl,
				size: Vec2f::new(persona_size.x(), persona_text_height),
				border_color: Some(theme_color_1)
			})
			*/
		}
	];

	// The Spinitron windows update at the same rate as the shared update rate
	let spinitron_windows = make_spinitron_windows(
		&all_model_windows_info, shared_update_rate
	);

	////////// Making a streaming server status window

	let streaming_server_status_window = make_streaming_server_status_window(
		&api_keys.streaming_server_now_playing_url,
		update_rate_creator.new_instance(5.0), 3
	).await;

	////////// Making an error window

	let error_window = make_error_window(
		Vec2f::new(0.0, 0.95),
		Vec2f::new(0.15, 0.05),
		update_rate_creator.new_instance(1.0),
		WindowContents::Color(ColorSDL::RGBA(255, 0, 0, 90)),
		ColorSDL::RED
	);

	////////// Making a credit window

	let weather_and_credit_window_size = Vec2f::new(0.15, 0.03);
	let credit_border_and_text_color = ColorSDL::RGB(255, 153, 153);

	let num_commits = run_command("git", &["rev-list", "--count", "HEAD"])?;
	let branch_name = run_command("git", &["rev-parse", "--abbrev-ref", "HEAD"])?;
	let credit_message = format!("By Caspian Ahlberg, release #{num_commits}, on branch '{branch_name}'");

	let credit_window = make_credit_window(
		Vec2f::new(0.85, 0.97),
		weather_and_credit_window_size,
		credit_border_and_text_color,
		credit_border_and_text_color,
		credit_message
	);

	////////// Making a clock window

	let clock_size_x = 0.3;
	let clock_tl = Vec2f::new(1.0 - clock_size_x, 0.0);
	let clock_size = Vec2f::new(clock_size_x, 1.0);

	let clock_dial_creation_info = TextureCreationInfo::from_path_async("assets/watch_dial.png").await?;

	let (clock_hands, _) = ClockHands::new_with_window(
		UpdateRate::ONCE_PER_FRAME,
		clock_tl,
		clock_size,

		ClockHandConfigs {
			milliseconds: ClockHandConfig::new(0.01, 0.2, 0.5, ColorSDL::RGBA(255, 0, 0, 100)), // Milliseconds
			seconds: ClockHandConfig::new(0.01, 0.02, 0.48, ColorSDL::WHITE), // Seconds
			minutes: ClockHandConfig::new(0.01, 0.02, 0.35, ColorSDL::YELLOW), // Minutes
			hours: ClockHandConfig::new(0.01, 0.02, 0.2, ColorSDL::BLACK) // Hours
		},

		WindowContents::make_texture_contents(&clock_dial_creation_info, texture_pool)?
	)?;

	////////// Defining the data for the surprise window

	let surprises = &[
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
			texture_blend_mode: BlendMode::Add,

			update_rate: Duration::milliseconds(100),
			num_update_steps_to_appear_for: 9,
			chance_of_appearing_when_updating: 0.0, // This one can only be triggered artificially

			local_hours_24_start: 0,
			local_hours_24_end: 23,

			flicker_window: true
		},

		SurpriseCreationInfo {
			texture_path: "assets/hintze.jpg",
			texture_blend_mode: BlendMode::None,

			update_rate: Duration::milliseconds(800),
			num_update_steps_to_appear_for: 10,
			chance_of_appearing_when_updating: 0.00001,

			local_hours_24_start: 10,
			local_hours_24_end: 20,

			flicker_window: true
		}
	];

	////////// Defining the Spinitron state parametwrs

	let initial_spin_window_size_guess = (1024, 1024);

	let custom_model_expiry_durations = [
		Duration::minutes(10), // 10 minutes after a spin, it's expired
		Duration::minutes(-5), // 5 minutes before a playlist ends, let the DJ know that they should pack up
		Duration::minutes(0), // Personas don't expire (their end time is the max UTC time)
		Duration::minutes(0) // 0 minutes after a show, it's expired (this is not used in practice)
	];

	////////// Defining some static texture info

	// Texture path, top left, size (TODO: make animated textures possible)
	let main_static_texture_info = [];
	let foreground_static_texture_info = [];
	let background_static_texture_info = [];

	////////// Making couple of different window types (and other stuff) concurrently

	let (weather_window, surprise_window, spinitron_state, twilio_state,
		background_static_texture_creation_info,
		foreground_static_texture_creation_info,
		main_static_texture_creation_info) = tokio::try_join!(

		make_weather_window(
			&api_keys.tomorrow_io,
			update_rate_creator,

			Vec2f::new(0.0, 1.0 - weather_and_credit_window_size.y()),
			weather_and_credit_window_size,

			theme_color_1, theme_color_1,
			WindowContents::Nothing,

			Some(RemakeTransitionInfo::new(
				Duration::seconds(1),
				easing_fns::transition::opacity::STRAIGHT_WAVY,
				easing_fns::transition::aspect_ratio::STRAIGHT_WAVY
			))
		),

		make_surprise_window(
			Vec2f::ZERO, Vec2f::ONE, "/tmp/surprises_wbor_studio_dashboard.sock",
			surprises, update_rate_creator, texture_pool
		),

		SpinitronState::new(
			(&api_keys.spinitron, get_fallback_texture_creation_info,
			custom_model_expiry_durations, initial_spin_window_size_guess)
		),

		TwilioState::new(
			&api_keys.twilio_account_sid,
			&api_keys.twilio_auth_token,
			11,
			Duration::days(5),
			false,

			Some(RemakeTransitionInfo::new(
				Duration::seconds(2),
				easing_fns::transition::opacity::BURST_BLENDED_BOUNCE,
				easing_fns::transition::aspect_ratio::BOUNCE
			))
		),

		make_creation_info_for_static_texture_set(&background_static_texture_info),
		make_creation_info_for_static_texture_set(&foreground_static_texture_info),
		make_creation_info_for_static_texture_set(&main_static_texture_info)
	)?;

	////////// Making a Twilio window

	let twilio_window = make_twilio_window(
		&twilio_state,
		shared_update_rate,

		Vec2f::new(0.58, 0.40),
		Vec2f::new(0.4, 0.55),

		0.025,
		WindowContents::Color(ColorSDL::RGB(23, 23, 23)),

		Vec2f::new(0.0, 0.45),
		Some(theme_color_1),  ColorSDL::RGB(238, 238, 238),

		WindowContents::Nothing
	);

	////////// Making some static texture windows

	let mut all_main_windows = vec![twilio_window, weather_window, credit_window, streaming_server_status_window];
	all_main_windows.extend(spinitron_windows);
	add_static_texture_set(&mut all_main_windows, &main_static_texture_info, &main_static_texture_creation_info, texture_pool);

	/* The error window goes last (so that it can manage
	errors in one shared update iteration properly) */
	all_main_windows.push(error_window);

	////////// Making all of the main windows

	// Modify the calculation of the main window's position and size
	let (main_window_tl_y, main_window_size_y) = (0.0, 1.0);

	// Calculate the x width for the main window
	let x_width_from_main_window_gap_size = 1.0 - main_windows_gap_size * 2.0;

	let mut main_window = Window::new(
		vec![],
		DynamicOptional::NONE,

		WindowContents::Many(
			background_static_texture_creation_info.iter().map(|info|
				WindowContents::make_texture_contents(info, texture_pool).unwrap()
			).collect()
		),

		Some(theme_color_1),
		Vec2f::new(main_windows_gap_size, main_window_tl_y),
		Vec2f::new(x_width_from_main_window_gap_size, main_window_size_y),
		all_main_windows
	);

	main_window.set_aspect_ratio_correction_skipping(true);

	////////// Making the highest-level window

	let mut all_windows = vec![main_window];
	add_static_texture_set(&mut all_windows, &foreground_static_texture_info, &foreground_static_texture_creation_info, texture_pool);
	all_windows.push(surprise_window);

	let all_windows_window = Window::new(
		vec![],
		DynamicOptional::NONE,
		WindowContents::Color(ColorSDL::RGB(23, 23, 23)),
		None,
		Vec2f::ZERO,
		Vec2f::ONE,
		all_windows
	);

	////////// Defining the shared state

	let boxed_shared_state = DynamicOptional::new(
		SharedWindowState {
			clock_hands,
			spinitron_state,
			error_state: ErrorState::new(),
			twilio_state,
			font_info: &FONT_INFO,
			get_fallback_texture_creation_info,
			rand_generator: rand::thread_rng()
		}
	);

	//////////

	Ok((
		all_windows_window,
		boxed_shared_state
	))
}
