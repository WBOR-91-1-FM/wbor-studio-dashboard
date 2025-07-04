use sdl2::ttf::{FontStyle, Hinting};

use crate::{
	themes::shared_utils::*,
	spinitron::{model::SpinitronModelName, state::SpinitronState},

	texture::{
		text::FontInfo,
		pool::{TextureCreationInfo, TexturePool, RemakeTransitionInfo}
	},

	utils::{
		request,
		vec2f::Vec2f,
		time::Duration,
		generic_result::*,
		dynamic_optional::DynamicOptional,
		update_rate::{UpdateRate, UpdateRateCreator}
	},

	window_tree::{
		Window,
		ColorSDL,
		WindowContents,
		TypicalWindowParams
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

	request::init_client(); // Initializing the shared client

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

	let top_bar_window_size_y = 0.1;
	let main_windows_gap_size = 0.01;

	let mut rand_generator = rand::thread_rng();
	let api_keys = ApiKeys::new().await?;

	let (theme_color_1, theme_border_radius_1) = (ColorSDL::RGB(249, 236, 210), 8);
	let theme_border_info_1 = Some((theme_color_1, theme_border_radius_1));

	let shared_api_update_rate = Duration::seconds(15);
	let weather_api_update_rate = Duration::minutes(10);
	let streaming_server_status_api_update_rate = Duration::seconds(30);

	let shared_view_refresh_update_rate = update_rate_creator.new_instance(0.25);
	let weather_view_refresh_update_rate = update_rate_creator.new_instance(60.0); // Once per minute

	////////// Defining the Spinitron window extents

	// Note: `tl` = top left
	let spin_tl = Vec2f::new_scalar(main_windows_gap_size);
	let spin_size = Vec2f::new_scalar(0.55);
	let spin_tr = spin_tl.x() + spin_size.x();

	let spin_text_height = 0.03;
	let spin_text_tl = Vec2f::translate_y(&spin_tl, spin_size.y());
	let spin_text_size = Vec2f::new(spin_size.x(), spin_text_height);

	let persona_tl = Vec2f::new(spin_tr + main_windows_gap_size, spin_tl.y());
	let persona_size = Vec2f::new_scalar(0.1);

	let persona_text_tl = Vec2f::translate_y(&persona_tl, persona_size.y());
	let persona_text_height = 0.02;

	let playlist_text_tl = Vec2f::translate(&(spin_tl + spin_size), 0.03, -0.2);
	let playlist_text_size = Vec2f::new(0.37, 0.05);

	// Most times, this is just the show image
	let playlist_tl = Vec2f::new(persona_tl.x() + persona_size.x() + main_windows_gap_size, spin_tl.y());
	let playlist_size = Vec2f::new_scalar(1.0 - playlist_tl.x() - main_windows_gap_size);

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
				size: Vec2f::new(persona_size.x(), persona_text_height),
				border_info: theme_border_info_1
			})
		}
	];

	// The Spinitron windows update at the same rate as the shared update rate
	let spinitron_windows = make_spinitron_windows(
		TypicalWindowParams {
			view_refresh_update_rate: shared_view_refresh_update_rate,
			border_info: None,
			top_left: Vec2f::new(0.25, 0.73),
			size: Vec2f::new(0.5, 0.11)
		},

		&all_model_windows_info,
		num_spins_shown_in_history,
		&mut rand_generator
	);

	////////// Making an error window

	let error_window = make_error_window(
		TypicalWindowParams {
			view_refresh_update_rate: shared_view_refresh_update_rate,
			border_info: None,
			top_left: Vec2f::new(0.0, 0.95),
			size: Vec2f::new(0.15, 0.05)
		},

		WindowContents::Color(ColorSDL::RGBA(255, 0, 0, 180)),
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
	let main_static_texture_info = [
		("assets/dashboard_bookshelf.png", Vec2f::ZERO, Vec2f::ONE, false),
		// ("assets/logo.png", Vec2f::new(0.6, 0.75), Vec2f::new(0.1, 0.05), false),
		("assets/soup.png", Vec2f::new(0.45, 0.72), Vec2f::new(0.06666666, 0.1), false),
		("assets/ness.bmp", Vec2f::new(0.28, 0.73), Vec2f::new_scalar(0.08), false)
	];

	let foreground_static_texture_info = [
		("assets/dashboard_foreground.png", Vec2f::ZERO, Vec2f::ONE, true)
	];

	let background_static_texture_info = [];

	////////// Making couple of different window types (and other stuff), some concurrently

	let streaming_server_status_window = make_streaming_server_status_window(
		&api_keys.streaming_server_now_playing_url,
		streaming_server_status_api_update_rate,
		shared_view_refresh_update_rate, 3
	);

	let weather_window = make_weather_window(
		TypicalWindowParams {
			view_refresh_update_rate: weather_view_refresh_update_rate,

			border_info: theme_border_info_1,
			top_left: Vec2f::ZERO,
			size: Vec2f::new(0.4, 0.3)
		},

		&api_keys.tomorrow_io,
		weather_api_update_rate,
		theme_color_1,
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
			(&api_keys.spinitron, get_fallback_texture_path,
			shared_api_update_rate, custom_model_expiry_durations, initial_spin_window_size_guess,
			initial_spin_history_subwindow_size_guess, num_spins_shown_in_history)
		),

		TwilioState::new(
			shared_api_update_rate,
			(&api_keys.twilio_account_sid, &api_keys.twilio_auth_token),

			6,
			Duration::days(5),
			false,

			theme_color_1,

			Some(RemakeTransitionInfo::new(
				Duration::seconds(2),
				easing_fns::transition::opacity::BURST_BLENDED_BOUNCE,
				easing_fns::transition::aspect_ratio::BOUNCE
			))
		),

		TextureCreationInfo::from_path_async("assets/text_bubble.png"),
		TextureCreationInfo::from_path_async("assets/watch_dial.png"),

		make_creation_info_for_static_texture_set(&background_static_texture_info),
		make_creation_info_for_static_texture_set(&foreground_static_texture_info),
		make_creation_info_for_static_texture_set(&main_static_texture_info)
	)?;

	////////// Making a Twilio window

	let twilio_windows = make_twilio_windows(
		TypicalWindowParams {
			view_refresh_update_rate: shared_view_refresh_update_rate,
			border_info: theme_border_info_1,
			top_left: Vec2f::new(0.58, 0.45),
			size: Vec2f::new(0.4, 0.27)
		},

		&twilio_state,

		0.025,
		WindowContents::Color(ColorSDL::RGB(0, 200, 0)),
		Vec2f::new(0.1, 0.45),
		WindowContents::make_texture_contents(&twilio_message_background_contents_creation_info, texture_pool)?
	);

	////////// Making a credit window

	let credit_message = format!("By Caspian Ahlberg, release #{num_commits}, on branch '{branch_name}'"); // TODO: include the theme name in this?

	let credit_window = make_credit_window(
		Vec2f::new(0.85, 0.97),
		Vec2f::new(0.15, 0.03),
		Some((ColorSDL::RED, theme_border_radius_1)),
		ColorSDL::RGB(210, 180, 140),
		credit_message
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

		WindowContents::make_texture_contents(&clock_dial_creation_info, texture_pool)?
	)?;

	////////// Making some static texture windows

	let mut all_main_windows = vec![credit_window, streaming_server_status_window];
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
		vec![clock_window, weather_window]
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
		WindowContents::Color(ColorSDL::RGB(0, 128, 128)),
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
