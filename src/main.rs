mod request;
mod texture;
mod spinitron;
mod window_tree;
mod utility_types;
mod window_tree_defs;

/*
Worked from this in the beginning: https://blog.logrocket.com/using-sdl2-bindings-rust/

TODO:

- Features:
	- Avoid screen burn-in somehow
	- DJ tips popping up now and then (like a video game loading screen)
	- A 'text the DJ' feature
	- Display streaming server online status (determined by whether it pings?) address is: 161.35.248.7
	- User interaction with the dashboard via the Stream Deck (toggle display elements, ignore DJ text, etc.)
	- Tell DJs in realtime whether they need to increase or decrease the volume of their input
	- Display the last 5 text messages, instead of just the last one
- Technical:
	- Maybe put the bounding box definition one layer out (with the parent)
	- Abstract the main loop out, so that just some data and fns are passed into it
	- Check for no box intersections
	- Put the box definitions in a JSON file
	- Eventually, avoid all possibilities of panics (so all assertions and unwraps should be gone)
	- When an error happens, make it print a message on screen that says that they should email me (make a log of the error on disk too)
	- Maybe draw rounded rectangles with `sdl_gfx` later on
	- Render a text drop shadow
	- Set more rendering hints later on, if needed (beyond just the scale quality)
	- If useful at some point, perhaps cut off rendered text characters with '...' if the text is too long
	- Figure out how to do pixel-size-independent-rendering (use `sdl_canvas.set_scale` for that?)
	- If possible, figure out how to use the extra wasted space lost when doing aspect ratio correction
- Fun ideas:
	- Maybe give a retro theme to everything
	- Some little Mario-type character running around the edges of the screen (like 'That Editor' by Bisqwit)
	- When the studio door opens and a show is over, display the expected person's name, saying 'welcome, _', until they scrobble any songs
	- Different themes per each dashboard setup: wooden, garden, neon retro, frutiger aero, etc.
	- Fall: leaves + drifting clouds over the screen, summer: shining run rays, spring: occasional rain with sun, winter: snow
	- Subway Surfers gameplay somewhere on screen?
- Misc:
	- There is an initial screen flicker on MacOS upon startup (and randomly when running), for some reason
*/

// https://gamedev.stackexchange.com/questions/137882/
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

struct AppConfig<'a> {
	name: &'a str,
	screen_option: ScreenOption,
	hide_cursor: bool,
	use_linear_filtering: bool,
	bg_color: window_tree::ColorSDL,
	icon_path: &'a str,

	top_level_window_creator: fn(
		&mut texture::TexturePool,
		(u32, u32),
		utility_types::update_rate::UpdateRateCreator
	)
		-> utility_types::generic_result::GenericResult<(
			window_tree::Window, utility_types::dynamic_optional::DynamicOptional,
			window_tree::PossibleSharedWindowStateUpdater)>
}

fn get_fps(sdl_timer: &sdl2::TimerSubsystem,
	sdl_prev_performance_counter: u64,
	sdl_performance_frequency: u64) -> f64 {

	let delta_time = sdl_timer.performance_counter() - sdl_prev_performance_counter;
	sdl_performance_frequency as f64 / delta_time as f64
}

fn main() -> utility_types::generic_result::GenericResult<()> {
	/* TODO: maybe artificially lower the FPS to reduce
	stress on the Pi, if a high framerate isn't needed later on.
	Maybe make the FPS equate with the highest poll rate, eventually? */

	/* TODO: make this more configurable, somehow
	(maybe make a SDL window init fn, where I pass in state?) */
	let app_config = AppConfig {
		name: "WBOR Studio Dashboard",

		screen_option: ScreenOption::Windowed(800, 800, false, None),
		// screen_option: ScreenOption::FullscreenDesktop,
		// screen_option: ScreenOption::Fullscreen,

		hide_cursor: true,
		use_linear_filtering: true,
		bg_color: window_tree::ColorSDL::RGB(50, 50, 50),
		icon_path: "assets/wbor_plane.bmp",
		top_level_window_creator: window_tree_defs::window_tree_defs::make_wbor_dashboard
	};

	//////////

	let sdl_context = sdl2::init()?;
	let sdl_video_subsystem = sdl_context.video()?;
	let mut sdl_event_pump = sdl_context.event_pump()?;

	use sdl2::video::WindowBuilder;

	let build_window = |width: u32, height: u32, applier: fn(&mut WindowBuilder) -> &mut WindowBuilder|
		applier(&mut sdl_video_subsystem.window(app_config.name, width, height)).allow_highdpi().opengl().build();

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
			let mode = sdl_video_subsystem.desktop_display_mode(0)?;

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
			println!("Window translucency not supported by your current platform! Official error: '{}'.", err);
		}
	}

	use sdl2::image::LoadSurface;
	sdl_window.set_icon(sdl2::surface::Surface::from_file(app_config.icon_path)?);

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

	let sdl_timer = sdl_context.timer()?;
	let sdl_performance_frequency = sdl_timer.performance_frequency();
	let sdl_ttf_context = sdl2::ttf::init()?;

	let texture_creator = sdl_canvas.texture_creator();

	let fps = sdl_video_subsystem.current_display_mode(0)?.refresh_rate as u16;
	let output_size = sdl_canvas.output_size()?;

	let mut rendering_params =
		window_tree::PerFrameConstantRenderingParams {
			sdl_canvas,
			texture_pool: texture::TexturePool::new(&texture_creator, &sdl_ttf_context),
			frame_counter: utility_types::update_rate::FrameCounter::new(),
			shared_window_state: utility_types::dynamic_optional::DynamicOptional::NONE,
			shared_window_state_updater: None
		};

	let (mut top_level_window, shared_window_state, shared_window_state_updater) =
		(app_config.top_level_window_creator)(
			&mut rendering_params.texture_pool,
			output_size,
			utility_types::update_rate::UpdateRateCreator::new(fps)
		)?;

	rendering_params.shared_window_state = shared_window_state;
	rendering_params.shared_window_state_updater = shared_window_state_updater;

	//////////

	let mut initial_num_textures_in_pool = None;

	'running: loop {
		for sdl_event in sdl_event_pump.poll_iter() {
			use sdl2::{event::Event, keyboard::Keycode};

			match sdl_event {
				Event::Quit {..} | Event::KeyDown {keycode: Some(Keycode::Escape), ..} => break 'running,
				_ => {}
			}
		}

		// TODO: should I put this before event polling?
		let sdl_performance_counter_before = sdl_timer.performance_counter();

		if let Some((shared_window_state_updater, shared_update_rate)) = shared_window_state_updater {
			if shared_update_rate.is_time_to_update(rendering_params.frame_counter) {
				shared_window_state_updater(&mut rendering_params.shared_window_state)?;
			}
		}

		//////////

		rendering_params.sdl_canvas.set_draw_color(app_config.bg_color); // TODO: remove eventually
		rendering_params.sdl_canvas.clear(); // TODO: make this work on fullscreen too

		top_level_window.render(&mut rendering_params)?;

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

		////////// Checking for a possible texture memory leak

		let num_textures_in_pool = rendering_params.texture_pool.size();

		match initial_num_textures_in_pool {
			Some(initial_amount) => {
				if initial_amount != num_textures_in_pool {
					panic!("Memory leak! Texture pool grew by {} past the first frame", num_textures_in_pool - initial_amount);
				}
			},
			None => {
				initial_num_textures_in_pool = Some(num_textures_in_pool);
			}
		}

		//////////
	}

	Ok(())
}
