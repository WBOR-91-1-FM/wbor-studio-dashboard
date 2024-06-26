mod request;
mod texture;
mod spinitron;
mod window_tree;
mod utility_types;
mod dashboard_defs;

// Worked from this in the beginning: https://blog.logrocket.com/using-sdl2-bindings-rust/

// https://gamedev.stackexchange.com/questions/137882/
#[derive(serde::Deserialize)]
enum ScreenOption {
	/* This runs it as a small app window, which can optionally
	be borderless, and optionally be translucent too. */
	Windowed(u32, u32, bool, Option<f32>),

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
	icon_path: String,
	maybe_pause_subduration_ms_when_window_unfocused: Option<u32>,

	screen_option: ScreenOption,
	hide_cursor: bool,
	use_linear_filtering: bool,
	background_color: (u8, u8, u8)
}

fn get_fps(sdl_timer: &sdl2::TimerSubsystem,
	sdl_prev_performance_counter: u64,
	sdl_performance_frequency: u64) -> f64 {

	let delta_time = sdl_timer.performance_counter() - sdl_prev_performance_counter;
	sdl_performance_frequency as f64 / delta_time as f64
}

/*
fn check_for_texture_pool_memory_leak(initial_num_textures_in_pool: &mut Option<usize>, texture_pool: &texture::TexturePool) {
	let num_textures_in_pool = texture_pool.size();

	match initial_num_textures_in_pool {
		Some(initial_amount) => {
			if *initial_amount != num_textures_in_pool {
				let growth_amount = num_textures_in_pool - *initial_amount;
				panic!("Memory leak! Texture pool grew by {growth_amount} past the first frame.");
			}
		},
		None => {
			*initial_num_textures_in_pool = Some(num_textures_in_pool);
		}
	}
}
*/

fn main() -> utility_types::generic_result::MaybeError {
	env_logger::init();

	log::info!("App launched!");

	let app_config: AppConfig = utility_types::json_utils::load_from_file("assets/app_config.json")?;
	let top_level_window_creator = dashboard_defs::dashboard::make_dashboard;

	//////////

	use crate::utility_types::generic_result::ToGenericError;

	let sdl_context = sdl2::init().to_generic()?;
	let sdl_video_subsystem = sdl_context.video().to_generic()?;
	let mut sdl_event_pump = sdl_context.event_pump().to_generic()?;

	use sdl2::video::WindowBuilder;

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

	////////// Setting the window opacity and icon

	// TODO: why does not setting the opacity result in broken fullscreen screen clearing?
	if let ScreenOption::Windowed(.., Some(opacity)) = app_config.screen_option {
		if let Err(err) = sdl_window.set_opacity(opacity) {
			log::warn!("Window translucency not supported by your current platform! Official error: '{err}'.");
		}
	}

	use sdl2::image::LoadSurface;
	sdl_window.set_icon(sdl2::surface::Surface::from_file(app_config.icon_path).to_generic()?);

	//////////

	let sdl_canvas = sdl_window
		.into_canvas()
		.accelerated()
		.present_vsync()
		.build()?;

	//////////

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

	//////////

	let mut sdl_timer = sdl_context.timer().to_generic()?;
	let sdl_performance_frequency = sdl_timer.performance_frequency();
	let sdl_ttf_context = sdl2::ttf::init()?;

	let texture_creator = sdl_canvas.texture_creator();

	let fps = sdl_video_subsystem.current_display_mode(0).to_generic()?.refresh_rate as u32;

	let sdl_renderer_info = sdl_canvas.info();
	let max_texture_size = (sdl_renderer_info.max_texture_width, sdl_renderer_info.max_texture_height);

	let mut rendering_params =
		window_tree::PerFrameConstantRenderingParams {
			sdl_canvas,
			texture_pool: texture::TexturePool::new(&texture_creator, &sdl_ttf_context, max_texture_size),
			frame_counter: utility_types::update_rate::FrameCounter::new(),
			shared_window_state: utility_types::dynamic_optional::DynamicOptional::NONE,
			shared_window_state_updater: None
		};

	let core_init_info = (top_level_window_creator)(
		&mut rendering_params.texture_pool, utility_types::update_rate::UpdateRateCreator::new(fps)
	);

	let (mut top_level_window, shared_window_state, shared_window_state_updater) =
		match core_init_info {
			Ok(info) => info,
			Err(err) => panic!("An error arose when initializing the application: '{err}'.")
		};

	rendering_params.shared_window_state = shared_window_state;
	rendering_params.shared_window_state_updater = shared_window_state_updater;

	//////////

	let mut pausing_window = false;
	// let mut initial_num_textures_in_pool = None;

	log::info!("Finished setting up window. Canvas size: {:?}. Renderer info: {:?}.",
		rendering_params.sdl_canvas.output_size().to_generic()?, sdl_renderer_info);

	'running: loop {
		for sdl_event in sdl_event_pump.poll_iter() {
			use sdl2::{event::{self, Event}, keyboard::Keycode};

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

		//////////

		// TODO: should I put this before event polling?
		let sdl_performance_counter_before = sdl_timer.performance_counter();

		rendering_params.sdl_canvas.set_draw_color(app_config.background_color);
		rendering_params.sdl_canvas.clear(); // TODO: make this work on fullscreen too

		if let Err(err) = top_level_window.render(&mut rendering_params) {
			log::error!("An error arose during rendering: '{err}'."); // TODO: put this error in the red dialog on the screen (pass into the renderer)
		}

		if let Some((shared_window_state_updater, shared_update_rate)) = shared_window_state_updater {
			if shared_update_rate.is_time_to_update(rendering_params.frame_counter) {
				if let Err(err) = shared_window_state_updater(&mut rendering_params.shared_window_state, &mut rendering_params.texture_pool) {
					log::error!("An error arose from the shared window state updater: '{err}'."); // TODO: put this error in the red dialog on the screen
				}
			}
		}

		//////////

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

		// println!("fps without and with vsync = {:.3}, {:.3}", _fps_without_vsync, _fps_with_vsync);

		// TODO: add this back later
		// check_for_texture_pool_memory_leak(&mut initial_num_textures_in_pool, &rendering_params.texture_pool);
	}

	Ok(())
}
