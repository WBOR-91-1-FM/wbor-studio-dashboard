mod utils;
mod texture;
mod spinitron;
mod window_tree;
mod dashboard_defs;

use sdl2::{
	surface::Surface,
	keyboard::Keycode,
	image::LoadSurface,
	video::WindowBuilder,
	event::{self, Event}
};

use crate::{
	dashboard_defs::themes,
	window_tree::{ColorSDL, PixelAreaSDL},

	utils::{
		file_utils,
		generic_result::*,
		dynamic_optional::DynamicOptional,
		update_rate::{FrameCounter, UpdateRateCreator}
	}
};

//////////

// Worked from this in the beginning: https://blog.logrocket.com/using-sdl2-bindings-rust/

// https://gamedev.stackexchange.com/questions/137882/
#[derive(serde::Deserialize)]
enum ScreenOption {
	/* This runs it as a small app window, which can optionally
	be borderless, and optionally be translucent too. */
	Windowed(PixelAreaSDL, bool, Option<f64>),

	/* This allows you to switch windows without shutting
	down the app. It is slower than real fullscreen. */
	FullscreenDesktop,

	/* This makes the OS change its output rendering resolution to one of
	the officially supported ones (which you can find in your settings app).
	You cannot exit from this window while the app is still running. */
	Fullscreen
}

#[derive(serde::Deserialize)]
struct AppConfig {
	title: String,
	theme_name: String,
	icon_path: String,

	background_color: (u8, u8, u8),
	max_remake_transition_queue_size: usize,

	hide_cursor: bool,
	draw_borders: bool,
	use_linear_filtering: bool,
	window_always_on_top: bool,

	pause_subduration_ms_when_retrying_window_info_init: u32,
	maybe_pause_subduration_ms_when_window_unfocused: Option<u32>,

	screen_option: ScreenOption
}

//////////

fn get_fps(sdl_timer: &sdl2::TimerSubsystem,
	sdl_prev_performance_counter: u64,
	sdl_performance_frequency: u64) -> f64 {

	let delta_time = sdl_timer.performance_counter() - sdl_prev_performance_counter;
	sdl_performance_frequency as f64 / delta_time as f64
}

macro_rules! build_dashboard_theme {(
		$desired_theme_name:expr, $texture_pool:expr, $update_rate_creator:expr,
		[$($theme_module_name:ident),* $(,)?]) => {

		match $desired_theme_name {
			$(
				stringify!($theme_module_name) => {
					let function = dashboard_defs::themes::$theme_module_name::make_dashboard;
					function($texture_pool, $update_rate_creator).await
				}
			),*

			_ => panic!("Unrecognized dashboard theme: '{}'", $desired_theme_name)
		}
	};
}

//////////

#[tokio::main(flavor = "multi_thread")]
async fn main() {
	////////// Getting the beginning timestamp, starting the logger, and loading the app config

	let get_timestamp = || std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap();
	let time_before_launch = get_timestamp();

	env_logger::init();
	log::info!("App launched!");

	let app_config: AppConfig = file_utils::load_json_from_file("assets/app_config.json").await.unwrap();

	////////// Setting up SDL and the initial window

	let sdl_context = sdl2::init().unwrap();
	let sdl_video_subsystem = sdl_context.video().unwrap();
	let mut sdl_event_pump = sdl_context.event_pump().unwrap();

	let build_window = |size: PixelAreaSDL, applier: fn(&mut WindowBuilder) -> &mut WindowBuilder|
		applier(&mut sdl_video_subsystem.window(&app_config.title, size.0, size.1)).allow_highdpi().build();

	let mut sdl_window = match app_config.screen_option {
		ScreenOption::Windowed(size, borderless, _) => build_window(
			size,
			if borderless {|wb| wb.position_centered().borderless()}
			else {WindowBuilder::position_centered}
		),

		// The resolution passed in here is irrelevant
		ScreenOption::FullscreenDesktop => build_window(
			(0, 0),
			WindowBuilder::fullscreen_desktop
		),

		ScreenOption::Fullscreen => {
			let mode = sdl_video_subsystem.display_mode(0, 0).unwrap();

			build_window(
				(mode.w as _, mode.h as _),
				WindowBuilder::fullscreen
			)
		}
	}.unwrap();

	////////// Setting the window always-on-top state, opacity, and icon

	sdl_window.set_always_on_top(app_config.window_always_on_top);

	// TODO: why does not setting the opacity result in broken fullscreen screen clearing?
	if let ScreenOption::Windowed(.., Some(opacity)) = app_config.screen_option {
		if let Err(err) = sdl_window.set_opacity(opacity as f32) {
			log::warn!("Window translucency not supported by your current platform! Official error: '{err}'.");
		}
	}

	sdl_window.set_icon(Surface::from_file(app_config.icon_path).unwrap());

	////////// Making a SDL canvas

	let sdl_canvas = sdl_window
		.into_canvas()
		.accelerated()
		.present_vsync()
		.build().unwrap();

	////////// Setting the texture filtering option

	// TODO: why is the top-right texture not linearly filtered?
	let using_texture_filtering_option =
		sdl2::hint::set_with_priority(
			"SDL_RENDER_SCALE_QUALITY",
			if app_config.use_linear_filtering {"1"} else {"0"},
			&sdl2::hint::Hint::Override
		);

	assert!(using_texture_filtering_option);

	if app_config.hide_cursor {
		sdl_context.mouse().show_cursor(false);
	}

	////////// Setting up the SDL timer, the TTF context, and more

	let sdl_ttf_context = sdl2::ttf::init().unwrap();
	let texture_creator = sdl_canvas.texture_creator();
	let fps = sdl_video_subsystem.current_display_mode(0).unwrap().refresh_rate as u32;

	let sdl_renderer_info = sdl_canvas.info();
	let max_texture_size = (sdl_renderer_info.max_texture_width, sdl_renderer_info.max_texture_height);

	let sdl_timer = sdl_context.timer().unwrap();
	let sdl_performance_frequency = sdl_timer.performance_frequency();

	let texture_pool = texture::pool::TexturePool::new(
		&texture_creator, &sdl_ttf_context, max_texture_size, app_config.max_remake_transition_queue_size
	);

	let mut rendering_params =
		window_tree::PerFrameConstantRenderingParams {
			draw_borders: app_config.draw_borders,
			sdl_canvas,
			texture_pool,
			frame_counter: FrameCounter::new(),
			shared_window_state: DynamicOptional::NONE
		};

	let bg = app_config.background_color;
	let standard_background_color = ColorSDL::RGB(bg.0, bg.1, bg.2);

	log::info!("Canvas size: {:?}. Renderer info: {sdl_renderer_info:?}.", rendering_params.sdl_canvas.output_size().unwrap());
	log::info!("Finished setting up window. Launch time: {:?} ms.", (get_timestamp() - time_before_launch).as_millis());

	//////////

	let mut pausing_window = false;
	let mut maybe_top_level_window = None;
	// let mut initial_num_textures_in_pool = None;

	//////////

	// Displaying the canvas the first time around (so that it pops up quicker, while the core init info is still loading)
	rendering_params.sdl_canvas.present();

	'running: loop {
		////////// Doing some event polling.

		for sdl_event in sdl_event_pump.poll_iter() {
			match sdl_event {
				Event::Quit {..} | Event::KeyDown {keycode: Some(Keycode::Escape), ..} => break 'running,

				Event::Window {win_event, ..} => {
					match win_event {
						event::WindowEvent::FocusLost => pausing_window = true,
						event::WindowEvent::FocusGained => pausing_window = false,
						_ => {}
					}
				},

				_ => {}
			}
		}

		if pausing_window {
			if let Some(pause_subduration_ms) = app_config.maybe_pause_subduration_ms_when_window_unfocused {
				sdl_timer.delay(pause_subduration_ms);
				continue;
			}
		}

		////////// Initializing the top-level window and shared window state when needed.

		if maybe_top_level_window.is_none() {
			let time_before_making_core_init_info = get_timestamp();

			let core_init_info = build_dashboard_theme!(
				app_config.theme_name.as_str(), &mut rendering_params.texture_pool,
				UpdateRateCreator::new(fps), [standard, barebones, retro_room]
			);

			match core_init_info {
				Ok((inited_top_level_window, shared_window_state)) => {
					log::info!("Time to build core init info: {:?} ms.", (get_timestamp() - time_before_making_core_init_info).as_millis());
					maybe_top_level_window = Some(inited_top_level_window);
					rendering_params.shared_window_state = shared_window_state;
				}

				Err(err) => {
					log::error!("Error with initializing the core init info: '{err}'. Waiting a bit, and then trying again shortly.");
					sdl_timer.delay(app_config.pause_subduration_ms_when_retrying_window_info_init);
					continue;
				}
			}
		}

		////////// Rendering the top-level window.

		// TODO: should I put this before event polling?
		let sdl_performance_counter_before = sdl_timer.performance_counter();

		rendering_params.sdl_canvas.set_draw_color(standard_background_color);
		rendering_params.sdl_canvas.clear(); // TODO: make this work on fullscreen too (on MacOS)

		maybe_top_level_window.as_mut().unwrap().render(&mut rendering_params);
		rendering_params.frame_counter.tick();

		let _fps_without_vsync = get_fps(&sdl_timer,
			sdl_performance_counter_before,
			sdl_performance_frequency
		);

		rendering_params.sdl_canvas.present();

		let _fps_with_vsync = get_fps(&sdl_timer,
			sdl_performance_counter_before,
			sdl_performance_frequency
		);

		// println!("fps with and without vsync = {:.3}, {:.3}", _fps_with_vsync, _fps_without_vsync);
	}
}
