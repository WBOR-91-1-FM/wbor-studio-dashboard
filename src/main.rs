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
- Set an update frequency rate for certain sindows (will update a certain number of times over a second)
*/

struct AppConfig<'a> {
	name: &'a str,
	width: u32,
	height: u32,
	bg_color: window_tree::ColorSDL
}

fn get_fps(sdl_timer: &sdl2::TimerSubsystem,
	sdl_prev_performance_counter: u64,
	sdl_performance_frequency: u64) -> f64 {

	let delta_time = sdl_timer.performance_counter() - sdl_prev_performance_counter;
	sdl_performance_frequency as f64 / delta_time as f64
}

fn main() -> utility_types::generic_result::GenericResult<()> {
	let config = AppConfig {
		name: "Recursive Box Demo",
		width: 800, height: 600, // This has the CRT aspect ratio
		bg_color: window_tree::ColorSDL::RGB(50, 50, 50)
	};

	let sdl_context = sdl2::init()?;
	let sdl_video_subsystem = sdl_context.video()?;

	let mut sdl_event_pump = sdl_context.event_pump()?;

	let sdl_window = sdl_video_subsystem
		.window(config.name, config.width, config.height)
		.position_centered()
		.opengl()
		.build()
		.map_err(|e| e.to_string())?;

	let mut sdl_canvas = sdl_window
		.into_canvas()
		.accelerated()
		.present_vsync()
		.build()
		.map_err(|e| e.to_string())?;

	let texture_creator = sdl_canvas.texture_creator();

	let (mut example_window, mut texture_pool) = window_tree_defs::make_example_window(&texture_creator)?;

	//////////

	let sdl_window_bounds = sdl2::rect::Rect::new(0, 0, config.width, config.height);
	let mut wrapping_frame_index = std::num::Wrapping(0);

	let sdl_timer = sdl_context.timer()?;
	let sdl_performance_frequency = sdl_timer.performance_frequency();

	'running: loop {
		for sdl_event in sdl_event_pump.poll_iter() {
			use sdl2::{event::Event, keyboard::Keycode};

			match sdl_event {
				Event::Quit {..} | Event::KeyDown {keycode: Some(Keycode::Escape), ..} => break 'running,
				_ => {}
			}
		}

		sdl_canvas.set_draw_color(config.bg_color); // TODO: remove eventually
		sdl_canvas.clear();

		let sdl_performance_counter_before = sdl_timer.performance_counter();

		example_window.render_recursively(
			&mut texture_pool,
			&mut sdl_canvas,
			wrapping_frame_index.0,
			sdl_window_bounds
		)?;

		//////////

		let _fps_without_vsync = get_fps(&sdl_timer,
			sdl_performance_counter_before,
			sdl_performance_frequency
		);

		//////////

		sdl_canvas.present();
		wrapping_frame_index += 1;

		let _fps_with_vsync = get_fps(&sdl_timer,
			sdl_performance_counter_before,
			sdl_performance_frequency
		);

		//////////

		/*
		println!("fps without and with vsync = {:.3}, {:.3}",
			fps_without_vsync, fps_with_vsync);
		*/

		//////////
	}

	Ok(())
}
