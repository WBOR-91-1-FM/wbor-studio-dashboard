mod request;
mod texture;
mod spinitron;
mod window_tree;
mod utility_types;
mod dashboard_defs;

use sdl2::{
	surface::Surface,
	keyboard::Keycode,
	image::LoadSurface,
	video::WindowBuilder,
	event::{self, Event}
};

use crate::{
	window_tree::ColorSDL,
	dashboard_defs::themes,

	utility_types::{
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
	Windowed(u32, u32, bool, Option<f64>),

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

	hide_cursor: bool,
	use_linear_filtering: bool,
	window_always_on_top: bool,
	pause_subduration_ms_when_retrying_window_info_init: u32,
	maybe_pause_subduration_ms_when_window_unfocused: Option<u32>,

	screen_option: ScreenOption,
}

//////////

fn get_fps(sdl_timer: &sdl2::TimerSubsystem,
	sdl_prev_performance_counter: u64,
	sdl_performance_frequency: u64) -> f64 {

	let delta_time = sdl_timer.performance_counter() - sdl_prev_performance_counter;
	sdl_performance_frequency as f64 / delta_time as f64
}

/*
fn check_for_texture_pool_memory_leak(initial_num_textures_in_pool: &mut Option<usize>, texture_pool: &texture::pool::TexturePool) {
	let num_textures_in_pool = texture_pool.size();

	match initial_num_textures_in_pool {
		Some(initial_amount) => {
			if *initial_amount != num_textures_in_pool {
				let growth_amount = num_textures_in_pool - *initial_amount;
				panic!("Memory leak! Texture pool grew by {growth_amount} past the first frame.");
			}
		},

		_ => {
			*initial_num_textures_in_pool = Some(num_textures_in_pool);
		}
	}
}
*/

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

const STANDARD_BACKGROUND_COLOR: ColorSDL = ColorSDL::BLACK;
const MAX_REMAKE_TRANSITION_QUEUE_SIZE: usize = 10; // This is to avoid unbounded memory consumption

#[tokio::main]
async fn main() -> MaybeError {
	////////// Getting the beginning timestamp, starting the logger, and loading the app config

	let get_timestamp = || std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
	let time_before_launch = get_timestamp()?;

	env_logger::init();
	log::info!("App launched!");

	let app_config: AppConfig = file_utils::load_json_from_file("assets/app_config.json").await?;

	////////// Setting up SDL and the initial window

	let sdl_context = sdl2::init().to_generic()?;
	let sdl_video_subsystem = sdl_context.video().to_generic()?;
	let mut sdl_event_pump = sdl_context.event_pump().to_generic()?;

	let build_window = |width: u32, height: u32, applier: fn(&mut WindowBuilder) -> &mut WindowBuilder|
		applier(&mut sdl_video_subsystem.window(&app_config.title, width, height)).allow_highdpi().build();

	let mut sdl_window = match app_config.screen_option {
		ScreenOption::Windowed(width, height, borderless, _) => build_window(
			width, height,
			if borderless {|wb| wb.position_centered().borderless()}
			else {WindowBuilder::position_centered}
		),

		// The resolution passed in here is irrelevant
		ScreenOption::FullscreenDesktop => build_window(
			0, 0, WindowBuilder::fullscreen_desktop
		),

		ScreenOption::Fullscreen => {
			let mode = sdl_video_subsystem.display_mode(0, 0).to_generic()?;

			build_window(
				mode.w as u32, mode.h as u32,
				WindowBuilder::fullscreen
			)
		}
	}?;

	////////// Setting the window always-on-top state, opacity, and icon

	sdl_window.set_always_on_top(app_config.window_always_on_top);

	// TODO: why does not setting the opacity result in broken fullscreen screen clearing?
	if let ScreenOption::Windowed(.., Some(opacity)) = app_config.screen_option {
		if let Err(err) = sdl_window.set_opacity(opacity as f32) {
			log::warn!("Window translucency not supported by your current platform! Official error: '{err}'.");
		}
	}

	sdl_window.set_icon(Surface::from_file(app_config.icon_path).to_generic()?);

	////////// Making a SDL canvas

	let sdl_canvas = sdl_window
		.into_canvas()
		.accelerated()
		.present_vsync()
		.build()?;

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

	let sdl_ttf_context = sdl2::ttf::init()?;
	let texture_creator = sdl_canvas.texture_creator();
	let fps = sdl_video_subsystem.current_display_mode(0).to_generic()?.refresh_rate as u32;

	let sdl_renderer_info = sdl_canvas.info();
	let max_texture_size = (sdl_renderer_info.max_texture_width, sdl_renderer_info.max_texture_height);

	let sdl_timer = sdl_context.timer().to_generic()?;
	let sdl_performance_frequency = sdl_timer.performance_frequency();

	let mut rendering_params =
		window_tree::PerFrameConstantRenderingParams {
			sdl_canvas,
			texture_pool: texture::pool::TexturePool::new(&texture_creator, &sdl_ttf_context, max_texture_size, MAX_REMAKE_TRANSITION_QUEUE_SIZE),
			frame_counter: FrameCounter::new(),
			shared_window_state: DynamicOptional::NONE
		};

	log::info!("Canvas size: {:?}. Renderer info: {sdl_renderer_info:?}.", rendering_params.sdl_canvas.output_size().to_generic()?);
	log::info!("Finished setting up window. Launch time: {:?} ms.", (get_timestamp()? - time_before_launch).as_millis());

	//////////

	let mut pausing_window = false;
	let mut maybe_top_level_window = None;
	// let mut initial_num_textures_in_pool = None;

	//////////

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

		////////// Initializing the top-level window and shared window state when needed. This also handles cases when the network is down upon launch.

		if maybe_top_level_window.is_none() {
			let time_before_making_core_init_info = get_timestamp()?;

			let core_init_info = build_dashboard_theme!(
				app_config.theme_name.as_str(), &mut rendering_params.texture_pool,
				UpdateRateCreator::new(fps), [standard, barebones, retro_room]
			);

			match core_init_info {
				Ok((inited_top_level_window, shared_window_state)) => {
					log::info!("Time to build core init info: {:?} ms.", (get_timestamp()? - time_before_making_core_init_info).as_millis());
					maybe_top_level_window = Some(inited_top_level_window);
					rendering_params.shared_window_state = shared_window_state;
				}

				Err(err) => {
					log::error!("Error with initializing the core init info: '{err}'. Waiting a bit, and then trying again shortly.");
					rendering_params.sdl_canvas.present();
					sdl_timer.delay(app_config.pause_subduration_ms_when_retrying_window_info_init);
					continue;
				}
			}
		}

		////////// Rendering the top-level window.

		// TODO: should I put this before event polling?
		let sdl_performance_counter_before = sdl_timer.performance_counter();

		rendering_params.sdl_canvas.set_draw_color(STANDARD_BACKGROUND_COLOR);
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

		//////////

		// TODO: add this back later
		// check_for_texture_pool_memory_leak(&mut initial_num_textures_in_pool, &rendering_params.texture_pool);
	}

	Ok(())
}
