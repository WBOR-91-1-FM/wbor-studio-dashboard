extern crate sdl2;

use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use std::time::Duration;

// Working from this: https://blog.logrocket.com/using-sdl2-bindings-rust/

struct WindowConfig<'a> {
	name: &'a str,
	width: u32,
	height: u32,
	fps: u32
}

pub fn main() -> Result<(), String> {
	let config = WindowConfig {
		name: "Plain Color Demo",
		width: 800,
		height: 600,
		fps: 60
	};

	let window_color = Color::RGB(255, 0, 0);

	let sdl_context = sdl2::init()?;
	let video_subsystem = sdl_context.video()?;

	let window = video_subsystem
		.window(config.name, config.width, config.height)
		.position_centered()
		.opengl()
		.build()
		.map_err(|e| e.to_string())?;

	let mut canvas = window.into_canvas().build().map_err(|e| e.to_string())?;

	canvas.set_draw_color(window_color);
	canvas.clear();
	canvas.present();
	let mut event_pump = sdl_context.event_pump()?;

	'running: loop {
		for event in event_pump.poll_iter() {
			match event {
				Event::Quit { .. }
				| Event::KeyDown {
					keycode: Some(Keycode::Escape),
					..
				} => break 'running,
				_ => {}
			}
		}

		canvas.clear();
		canvas.present();

		::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / config.fps));
		// The rest of the application loop goes here...
	}

	Ok(())
}
