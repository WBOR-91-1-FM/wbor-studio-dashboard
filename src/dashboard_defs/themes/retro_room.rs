use sdl2::ttf::{FontStyle, Hinting};

use crate::{
	themes::shared_utils::*,
	spinitron::{model::SpinitronModelName, state::SpinitronState},

	texture::{
		text::{FontInfo, HashableHinting},
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
		surprise::make_surprise_window,
		error::{make_error_window, ErrorState},
		shared_window_state::SharedWindowState,
		twilio::{make_twilio_window, TwilioState},
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
		hinting: HashableHinting(Hinting::Normal),
		maybe_outline_width: None
	};

	let top_bar_window_size_y = 0.0;
	let main_windows_gap_size = 0.0;

	let theme_color_1 = ColorSDL::RGB(134, 0, 41);
	let shared_update_rate = update_rate_creator.new_instance(30.0);
	let api_keys: ApiKeys = file_utils::load_json_from_file("assets/api_keys.json").await?;
	let mut rand_generator = rand::thread_rng();

	////////// Defining the Spinitron window extents

	// Note: `tl` = top left
	let spin_tl = Vec2f::new(0.045833327, 0.093749955);
	let spin_size = Vec2f::new_scalar(0.4);

	let spin_text_tl = Vec2f::new(0.04416666, 0.5412509);
	let spin_text_size = Vec2f::new(0.435, 0.045);

	let persona_tl =  Vec2f::new(0.94416624, 0.20249988);
	let persona_size = Vec2f::new_scalar(0.04);

	let persona_text_tl = Vec2f::new(0.92416626, 0.26349987);
	let persona_text_size = Vec2f::new(0.065, 0.018);

	let playlist_text_tl =  Vec2f::new(0.71916646, 0.45749995);
	let playlist_text_size = Vec2f::new(0.263, 0.018);

	// Most times, this is just the show image
	let playlist_tl = Vec2f::new(0.7341665, 0.23999995);
	let playlist_size = Vec2f::new_scalar(0.17);

	let num_spins_shown_in_history = 7;

	// TODO: make a type for the top-left/size combo (and add useful utility functions from there)

	////////// Making the Spinitron windows

	let all_model_windows_info = [
		SpinitronModelWindowsInfo {
			model_name: SpinitronModelName::Spin,
			text_color: theme_color_1,

			texture_window: Some(SpinitronModelWindowInfo {
				tl: spin_tl,
				size: spin_size,
				border_color: None
			}),

			text_window: Some(SpinitronModelWindowInfo {
				tl: spin_text_tl,
				size: spin_text_size,
				border_color: None
			})
		},

		SpinitronModelWindowsInfo {
			model_name: SpinitronModelName::Playlist,
			text_color: theme_color_1,

			texture_window: Some(SpinitronModelWindowInfo {
				tl: playlist_tl,
				size: playlist_size,
				border_color: None
			}),

			text_window: Some(SpinitronModelWindowInfo {
				tl: playlist_text_tl,
				size: playlist_text_size,
				border_color: None
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
				border_color: None
			}),

			text_window: Some(SpinitronModelWindowInfo {
				tl: persona_text_tl,
				size: persona_text_size,
				border_color: None
			})
		}
	];

	// The Spinitron windows update at the same rate as the shared update rate
	let spinitron_windows = make_spinitron_windows(
		&all_model_windows_info, shared_update_rate,

		Vec2f::new(0.43, 0.82),
		Vec2f::new(0.45, 0.15),
		None,

		num_spins_shown_in_history,
		&mut rand_generator
	);

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
		Vec2f::new(0.85, 0.0),
		weather_and_credit_window_size,
		credit_border_and_text_color,
		credit_border_and_text_color,
		credit_message
	);

	////////// Defining the Spinitron state parametwrs

	let initial_spin_window_size_guess = (1024, 1024);
	let initial_spin_history_subwindow_size_guess = (16, 16);

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

	let background_static_texture_info = [
		("assets/dashboard_background.png", Vec2f::ZERO, Vec2f::ONE, false)
	];

	////////// Making couple of different window types (and other stuff) concurrently

	let (streaming_server_status_window,
		weather_window, surprise_window, spinitron_state,
		twilio_state, twilio_message_background_contents_creation_info,
		clock_dial_creation_info,
		background_static_texture_creation_info,
		foreground_static_texture_creation_info,
		main_static_texture_creation_info) = tokio::try_join!(

		async {Ok(make_streaming_server_status_window(
			&api_keys.streaming_server_now_playing_url,
			update_rate_creator.new_instance(5.0), 3
		).await)},

		make_weather_window(
			&api_keys.tomorrow_io,
			update_rate_creator,

			Vec2f::new(0.008, 0.01),
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
			Vec2f::ZERO, Vec2f::ONE, "surprises",
			&ALL_SURPRISES, update_rate_creator, texture_pool
		),

		SpinitronState::new(
			(&api_keys.spinitron, get_fallback_texture_creation_info,
			custom_model_expiry_durations, initial_spin_window_size_guess,
			initial_spin_history_subwindow_size_guess, num_spins_shown_in_history,

			Some(RemakeTransitionInfo::new(
				Duration::seconds(1),
				easing_fns::transition::opacity::STRAIGHT_WAVY,
				easing_fns::transition::aspect_ratio::STRAIGHT_WAVY
			)))
		),

		TwilioState::new(
			&api_keys.twilio_account_sid,
			&api_keys.twilio_auth_token,
			3,
			Duration::days(5),
			false,

			Some(RemakeTransitionInfo::new(
				Duration::seconds(2),
				easing_fns::transition::opacity::BURST_BLENDED_BOUNCE,
				easing_fns::transition::aspect_ratio::BOUNCE
			))
		),

		TextureCreationInfo::from_path_async("assets/alternative_text_bubble.png"),
		TextureCreationInfo::from_path_async("assets/watch_dial.png"),

		make_creation_info_for_static_texture_set(&background_static_texture_info),
		make_creation_info_for_static_texture_set(&foreground_static_texture_info),
		make_creation_info_for_static_texture_set(&main_static_texture_info)
	)?;

	////////// Making a Twilio window

	let twilio_window = make_twilio_window(
		&twilio_state,
		shared_update_rate,
		update_rate_creator.new_instance(1.0),

		Vec2f::new(0.69, 0.52),
		Vec2f::new(0.3, 0.06),

		0.015,
		WindowContents::Color(ColorSDL::RGBA(0, 240, 0, 80)),

		Vec2f::new(0.075, 0.45),
		None, theme_color_1,

		WindowContents::make_texture_contents(&twilio_message_background_contents_creation_info, texture_pool)?
	);

	////////// Making a clock window

	let clock_tl = Vec2f::new(0.46000004, 0.020000035);
	let clock_size = Vec2f::new_scalar(0.1);

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

		WindowContents::make_texture_contents(&clock_dial_creation_info, texture_pool)?
	)?;

	////////// Making some static texture windows

	let mut all_main_windows = Vec::new();

	all_main_windows.extend(spinitron_windows);
	all_main_windows.extend([twilio_window, credit_window, clock_window, weather_window, streaming_server_status_window]);
	add_static_texture_set(&mut all_main_windows, &main_static_texture_info, &main_static_texture_creation_info, texture_pool);

	/* The error window goes last (so that it can manage
	errors in one shared update iteration properly) */
	all_main_windows.push(error_window);

	////////// Making all of the main windows

	let main_window_tl_y = main_windows_gap_size + top_bar_window_size_y + main_windows_gap_size;
	let main_window_size_y = 1.0 - main_window_tl_y - main_windows_gap_size;
	let x_width_from_main_window_gap_size = 1.0 - main_windows_gap_size * 2.0;

	let top_bar_tl = Vec2f::new_scalar(main_windows_gap_size);

	let top_bar_window = Window::new(
		vec![],
		DynamicOptional::NONE,
		WindowContents::Color(ColorSDL::RGB(128, 0, 32)),
		None,
		top_bar_tl,
		Vec2f::new(x_width_from_main_window_gap_size, top_bar_window_size_y),
		vec![]
	);

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

	let mut all_windows = vec![top_bar_window, main_window];
	add_static_texture_set(&mut all_windows, &foreground_static_texture_info, &foreground_static_texture_creation_info, texture_pool);
	all_windows.push(surprise_window);

	let all_windows_window = Window::new(
		vec![],
		DynamicOptional::NONE,
		WindowContents::Nothing,
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
			rand_generator
		}
	);

	//////////

	Ok((
		all_windows_window,
		boxed_shared_state
	))
}
