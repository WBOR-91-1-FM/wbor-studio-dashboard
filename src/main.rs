extern crate sdl2;

use std::time::Duration;

use sdl2::{
	event::Event, pixels::Color,
	keyboard::Keycode,
	rect::Rect
};

pub mod window_hierarchy;

use window_hierarchy::{
	Vec2f, WindowContents,
	HierarchalWindow, render_windows_recursively
};

// Working from this: https://blog.logrocket.com/using-sdl2-bindings-rust/

struct AppConfig<'a> {
	name: &'a str,
	width: u32,
	height: u32,
	fps: u32,
	bg_color: Color
}

pub fn main() -> Result<(), String> {
	let config = AppConfig {
		name: "Recursive Box Demo",
		width: 800,
		height: 600,
		fps: 60,
		bg_color: Color::RGB(50, 50, 50)
	};

	let sdl_context = sdl2::init()?;
	let sdl_video_subsystem = sdl_context.video()?;

	let mut event_pump = sdl_context.event_pump()?;
	let sdl_window_bounds = Rect::new(0, 0, config.width, config.height);

	let sdl_window = sdl_video_subsystem
		.window(config.name, config.width, config.height)
		.position_centered().opengl().build()
		.map_err(|e| e.to_string())?;

	let mut sdl_canvas = sdl_window.into_canvas().build().map_err(|e| e.to_string())?;
	let texture_creator = sdl_canvas.texture_creator();

	sdl_canvas.set_draw_color(config.bg_color);
	sdl_canvas.clear();
	sdl_canvas.present();

	//////////

	/* TODO:
	- Maybe put the bounding box definition one layer out (with the parent)
	- Abstract the main loop out, so that just some data and fns are passed into it
	- check for no box intersections
	- Put the box definitions in a JSON file
	*/

	let bird = HierarchalWindow::new(
		WindowContents::make_texture("assets/bird.bmp", &texture_creator),
		Vec2f::new(0.1, 0.1),
		Vec2f::new(0.3, 0.9),
		None
	);

	let wbor_plane = HierarchalWindow::new(
		WindowContents::make_texture("assets/wbor_plane.bmp", &texture_creator),
		Vec2f::new(0.4, 0.1),
		Vec2f::new(0.7, 0.9),
		None
	);

	let photo_box = HierarchalWindow::new(
		WindowContents::make_transparent_color(0, 255, 0, 0.8),
		Vec2f::new(0.01, 0.01),
		Vec2f::new(0.75, 0.5),
		Some(vec![bird, wbor_plane])
	);

	let blue_box = HierarchalWindow::new(
		WindowContents::make_transparent_color(0, 0, 255, 1.0),
		Vec2f::new(0.1, 0.6),
		Vec2f::new(0.9, 0.9),
		None
	);

	let example_window = HierarchalWindow::new(
		WindowContents::make_color(255, 0, 0),
		Vec2f::new(0.01, 0.01),
		Vec2f::new(0.99, 0.99),
		Some(vec![photo_box, blue_box])
	);

	//////////

	'running: loop {
		for event in event_pump.poll_iter() {
			match event {
				| Event::Quit {..}
				| Event::KeyDown {keycode: Some(Keycode::Escape), ..} => break 'running,
				| _ => {}
			}
		}

		sdl_canvas.set_draw_color(config.bg_color); // TODO: remove this eventually
		sdl_canvas.clear();

		render_windows_recursively(&example_window, &mut sdl_canvas, sdl_window_bounds);

		sdl_canvas.present();

		::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / config.fps));
	}

	Ok(())
}
