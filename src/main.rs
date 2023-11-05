use sdl2;

mod request;
mod texture;
mod spinitron;
mod window_tree;
mod utility_types;
mod window_tree_defs;

/*
Worked from this in the beginning: https://blog.logrocket.com/using-sdl2-bindings-rust/

TODO:
- Maybe give a retro theme to everything
- Maybe put the bounding box definition one layer out (with the parent)
- Abstract the main loop out, so that just some data and fns are passed into it
- Check for no box intersections
- Put the box definitions in a JSON file
- Avoid screen burn-in somehow
- Eventually, avoid all possibilities of panics (so all assertions and unwraps should be gone)
- When an error happens, make it print a message on screen that says that they should email me (make a log of the error on disk too)
- When the studio door opens and a show is over, display the expected person's name, saying 'welcome, _', until they scrobble any songs
- Set an update frequency rate for certain windows (will update a certain number of times over a second)
- DJ tips popping up now and then (like a video game loading screen)
- Some little Mario-type character running around the edges of the screen (like 'That Editor' by Bisqit)
- A 'text the DJ' feature

- Text rendering
- Async requests (for that, make an async requester object that you can initiate a request with,
	and then make it possible to ask if it's ready yet - it should contain its asyncness within itself fully, if possible).
	See here: https://doc.rust-lang.org/std/future/trait.Future.html
*/

struct AppConfig<'a> {
	name: &'a str,
	width: u32,
	height: u32,
	bg_color: window_tree::ColorSDL,

	top_level_window_creator: fn(&mut texture::TexturePool)
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
	/* TODO: maybe artifically lower the FPS to reduce
	stress on the Pi, if a high framerate isn't needed later on.
	Maybe make the FPS equate with the highest poll rate, eventually? */

	/* TODO: make this more configurable, somehow
	(maybe make a SDL window init fn, where I pass in state?) */
	let app_config = AppConfig {
		name: "Recursive Box Demo",
		width: 800, height: 600, // This has the CRT aspect ratio
		bg_color: window_tree::ColorSDL::RGB(50, 50, 50),
		top_level_window_creator: window_tree_defs::make_example_window
	};

	//////////

	let sdl_context = sdl2::init()?;
	let sdl_video_subsystem = sdl_context.video()?;

	let mut sdl_event_pump = sdl_context.event_pump()?;

	let sdl_window = sdl_video_subsystem
		.window(app_config.name, app_config.width, app_config.height)
		.position_centered()
		.opengl()
		.build()
		.map_err(|e| e.to_string())?;

	let sdl_canvas = sdl_window
		.into_canvas()
		.accelerated()
		.present_vsync()
		.build()
		.map_err(|e| e.to_string())?;

	//////////

	let sdl_timer = sdl_context.timer()?;
	let sdl_performance_frequency = sdl_timer.performance_frequency();
	let sdl_window_bounds = sdl2::rect::Rect::new(0, 0, app_config.width, app_config.height);

	let texture_creator = sdl_canvas.texture_creator();

	let mut rendering_params =
		window_tree::PerFrameConstantRenderingParams {
			sdl_canvas,
			texture_pool: texture::TexturePool::new(&texture_creator),
			frame_counter: utility_types::update_rate::FrameCounter::new(),
			shared_window_state: utility_types::dynamic_optional::DynamicOptional::none(),
			shared_window_state_updater: None
		};

	let (mut top_level_window, shared_window_state, shared_window_state_updater) =
		(app_config.top_level_window_creator)(&mut rendering_params.texture_pool)?;

	rendering_params.shared_window_state = shared_window_state;
	rendering_params.shared_window_state_updater = shared_window_state_updater;

	//////////

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
		rendering_params.sdl_canvas.clear();

		top_level_window.render_recursively(&mut rendering_params, sdl_window_bounds)?;
		rendering_params.frame_counter.tick();

		//////////

		let _fps_without_vsync = get_fps(&sdl_timer,
			sdl_performance_counter_before,
			sdl_performance_frequency
		);

		//////////

		rendering_params.sdl_canvas.present();

		let _fps_with_vsync = get_fps(&sdl_timer,
			sdl_performance_counter_before,
			sdl_performance_frequency
		);

		//////////

		/*
		println!("fps without and with vsync = {:.3}, {:.3}",
			_fps_without_vsync, _fps_with_vsync);
		*/

		//////////
	}

	Ok(())
}
