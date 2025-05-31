use sdl2::ttf::{FontStyle, Hinting};

use crate::{
	themes::shared_utils::*,
	spinitron::{model::SpinitronModelName, state::SpinitronState},

	texture::{
		text::FontInfo,
		pool::{TextureCreationInfo, TexturePool, RemakeTransitionInfo}
	},

	utils::{
		file_utils,
		vec2f::Vec2f,
		time::Duration,
		generic_result::*,
		dynamic_optional::DynamicOptional,
		update_rate::{UpdateRate, UpdateRateCreator}
	},

	window_tree::{
		Window,
		ColorSDL,
		WindowContents
	},

	dashboard_defs::{
		easing_fns,
		credit::make_credit_window,
		weather::make_weather_window,
		surprise::make_surprise_window,
		error::{make_error_window, ErrorState},
		shared_window_state::SharedWindowState,
		twilio::{make_twilio_windows, TwilioState},
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

	const FONT_INFO: FontInfo = FontInfo::new(
		include_bytes!("../../../assets/unifont/unifont-15.1.05.otf"),
		include_bytes!("../../../assets/unifont/unifont_upper-15.1.05.otf"),

		/* Providing this function instead of the variant below since
		`font.find_glyph` is buggy for the Rust sdl2::ttf bindings */
		|_, c| c as u32 <= 65535,
		// |font, c| font.find_glyph(c).is_some(),

		FontStyle::NORMAL,
		Hinting::Normal,
		None
	);

	let top_bar_window_size_y = 0.0;
	let main_windows_gap_size = 0.0;

	let mut rand_generator = rand::thread_rng();
	let api_keys: ApiKeys = file_utils::load_json_from_file("assets/api_keys.json").await?;

	let theme_color_1 = ColorSDL::RGB(134, 0, 41);
	let theme_border_info_1 = None;

	let shared_api_update_rate = Duration::seconds(15);
	let weather_api_update_rate = Duration::minutes(10);
	let streaming_server_status_api_update_rate = Duration::seconds(30);

	let shared_view_refresh_update_rate = update_rate_creator.new_instance(0.25);
	let weather_view_refresh_update_rate = update_rate_creator.new_instance(60.0); // Once per minute

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
				border_info: theme_border_info_1
			}),

			text_window: Some(SpinitronModelWindowInfo {
				tl: spin_text_tl,
				size: spin_text_size,
				border_info: theme_border_info_1
			})
		},

		SpinitronModelWindowsInfo {
			model_name: SpinitronModelName::Playlist,
			text_color: theme_color_1,

			texture_window: Some(SpinitronModelWindowInfo {
				tl: playlist_tl,
				size: playlist_size,
				border_info: theme_border_info_1
			}),

			text_window: Some(SpinitronModelWindowInfo {
				tl: playlist_text_tl,
				size: playlist_text_size,
				border_info: theme_border_info_1
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
				border_info: theme_border_info_1
			}),

			text_window: Some(SpinitronModelWindowInfo {
				tl: persona_text_tl,
				size: persona_text_size,
				border_info: theme_border_info_1
			})
		}
	];

	// The Spinitron windows update at the same rate as the shared update rate
	let spinitron_windows = make_spinitron_windows(
		&all_model_windows_info, shared_view_refresh_update_rate,

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
		shared_view_refresh_update_rate,
		WindowContents::Color(ColorSDL::RGBA(255, 0, 0, 150)),
		ColorSDL::YELLOW
	);

	////////// Defining the Spinitron state parametwrs

	let initial_spin_window_size_guess = (1024, 1024);
	let initial_spin_history_subwindow_size_guess = (16, 16);

	let custom_model_expiry_durations = [
		Duration::minutes(10), // 10 minutes after a spin, it's expired
		Duration::minutes(-5), // 5 minutes before a playlist ends, let the DJ know that they should pack up
		Duration::minutes(0), // 0 minutes after a persona, it expires (behind the scenes, the start/end comes from the associated playlist)
		Duration::minutes(0) // 0 minutes after a show, it's expired (this is not used in practice)
	];

	////////// Defining some static texture info

	// Texture path, top left, size (TODO: make animated textures possible)
	let main_static_texture_info = [];
	let foreground_static_texture_info = [];

	let background_static_texture_info = [
		("assets/dashboard_background.png", Vec2f::ZERO, Vec2f::ONE, false)
	];

	////////// Making couple of different window types (and other stuff), some concurrently

	let streaming_server_status_window = make_streaming_server_status_window(
		&api_keys.streaming_server_now_playing_url,
		streaming_server_status_api_update_rate,
		shared_view_refresh_update_rate, 3
	);

	let weather_and_credit_window_size = Vec2f::new(0.15, 0.03);

	let weather_window = make_weather_window(
		&api_keys.tomorrow_io,
		weather_api_update_rate,
		weather_view_refresh_update_rate,

		Vec2f::new(0.008, 0.01),
		weather_and_credit_window_size,

		theme_color_1, Some((theme_color_1, 0)),
		WindowContents::Nothing,

		Some(RemakeTransitionInfo::new(
			Duration::seconds(1),
			easing_fns::transition::opacity::STRAIGHT_WAVY,
			easing_fns::transition::aspect_ratio::STRAIGHT_WAVY
		))
	);

	let (num_commits, branch_name, surprise_window,
		spinitron_state, twilio_state,
		twilio_message_background_contents_creation_info,
		clock_dial_creation_info,
		background_static_texture_creation_info,
		foreground_static_texture_creation_info,
		main_static_texture_creation_info) = tokio::try_join!(

		run_command("git", &["rev-list", "--count", "HEAD"]),
		run_command("git", &["rev-parse", "--abbrev-ref", "HEAD"]),

		make_surprise_window(
			Vec2f::ZERO, Vec2f::ONE, "surprises",
			&ALL_SURPRISES, update_rate_creator, texture_pool
		),

		SpinitronState::new(
			(&api_keys.spinitron, get_fallback_texture_creation_info,
			shared_api_update_rate, custom_model_expiry_durations, initial_spin_window_size_guess,
			initial_spin_history_subwindow_size_guess, num_spins_shown_in_history)
		),

		TwilioState::new(
			shared_api_update_rate,
			&api_keys.twilio_account_sid,
			&api_keys.twilio_auth_token,
			3,
			Duration::days(5),
			false,

			theme_color_1,

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

	let twilio_windows = make_twilio_windows(
		&twilio_state,
		shared_view_refresh_update_rate,

		Vec2f::new(0.69, 0.52),
		Vec2f::new(0.3, 0.06),

		0.015,
		WindowContents::Color(ColorSDL::RGBA(0, 240, 0, 80)),

		Vec2f::new(0.075, 0.45),
		None,

		WindowContents::make_texture_contents(&twilio_message_background_contents_creation_info, texture_pool)?
	);

	////////// Making a credit window

	let credit_border_and_text_color = ColorSDL::RGB(255, 153, 153);
	let credit_border_info = Some((credit_border_and_text_color, 0));
	let credit_message = format!("By Caspian Ahlberg, release #{num_commits}, on branch '{branch_name}'"); // TODO: include the theme name in this?

	let credit_window = make_credit_window(
		Vec2f::new(0.85, 0.0),
		weather_and_credit_window_size,
		credit_border_info,
		credit_border_and_text_color,
		credit_message
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

	let mut all_main_windows = vec![credit_window, streaming_server_status_window, weather_window, clock_window];
	add_static_texture_set(&mut all_main_windows, &main_static_texture_info, &main_static_texture_creation_info, texture_pool);

	all_main_windows.extend(spinitron_windows);
	all_main_windows.extend(twilio_windows);

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

		theme_border_info_1,
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
