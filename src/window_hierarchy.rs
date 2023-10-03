extern crate sdl2;

use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::Canvas;
use sdl2::video::Window;

// A 0-1 normalized floating-point vec2 (TODO: ONLY EVER USE THE CONSTRUCTOR)
pub struct Vec2f {
	x: f32, y: f32
}

impl Vec2f {
	pub fn new(x: f32, y: f32) -> Vec2f {
		std::assert!(x >= 0.0 && x <= 1.0);
		std::assert!(y >= 0.0 && y <= 1.0);
		Vec2f {x, y}
	}
}

pub enum WindowContents {
	PlainColor(Color), // Not using the alpha channel here
	// TODO: add more variants
}

pub struct HierarchalWindow {
	contents: WindowContents,
	top_left: Vec2f,
	bottom_right: Vec2f,

	/* TODO:
	- Maybe do some splitting thing here
	- Or include a list of children
	*/

	child: Option<Box<HierarchalWindow>>
}

impl HierarchalWindow {
	pub fn new(
		contents: WindowContents,
		top_left: Vec2f, bottom_right: Vec2f,
		child: Option<HierarchalWindow>) -> HierarchalWindow {

		std::assert!(top_left.x < bottom_right.x);
		std::assert!(top_left.y < bottom_right.y);

		let boxed_child = match child {
			Some(inner_child) => Some(Box::new(inner_child)),
			_ => None
		};

		HierarchalWindow {contents, top_left, bottom_right, child: boxed_child}
	}
}

pub fn render_windows_recursively(
	window: &HierarchalWindow,
	sdl_canvas: &mut Canvas<Window>,
	parent_rect: Rect) {

	let parent_width = parent_rect.width();
	let parent_height = parent_rect.height ();

	let origin_and_size = (
		window.top_left.x,
		window.top_left.y,
		window.bottom_right.x - window.top_left.x,
		window.bottom_right.y - window.top_left.y,
	);

	let rescaled_rect = Rect::new(
		(origin_and_size.0 * parent_width as f32) as i32 + parent_rect.x(),
		(origin_and_size.1 * parent_height as f32) as i32 + parent_rect.y(),
		(origin_and_size.2 * parent_width as f32) as u32,
		(origin_and_size.3 * parent_height as f32) as u32,
	);

	match window.contents {
		WindowContents::PlainColor(color) => {
			sdl_canvas.set_draw_color(color);
			let _ = sdl_canvas.fill_rect(rescaled_rect);
		},
	}

	if let Some(child) = &window.child {
		render_windows_recursively(child, sdl_canvas, rescaled_rect);
	}
}
