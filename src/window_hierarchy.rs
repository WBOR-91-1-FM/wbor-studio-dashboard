extern crate sdl2;

use sdl2::{
	pixels::Color, rect::Rect, video::{Window, WindowContext},
	render::{Canvas, Texture, TextureCreator, BlendMode}, surface::Surface
};

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

pub enum WindowContents<'a> {
	PlainColor(Color), // Not using the alpha channel here
	Texture(Texture<'a>)

	/*
	TODO: support these things:
	- An `UpdatableTexture` (i.e. an album cover) (or just extend the current `Texture` functionality)
	- An `UpdatableText` (pass in a font for that too)
	*/
}

impl WindowContents<'_> {
	pub fn make_color<'a>(r: u8, g: u8, b: u8) -> WindowContents<'a> {
		return WindowContents::PlainColor(Color::RGB(r, g, b));
	}

	// `a` ranges from 0 to 1
	pub fn make_transparent_color<'a>(r: u8, g: u8, b: u8, a: f32) -> WindowContents<'a> {
		std::assert!(a >= 0.0 && a <= 1.0);
		return WindowContents::PlainColor(Color::RGBA(r, g, b, (a * 255.0) as u8));
	}

	pub fn make_texture<'a>(path: &'a str,
		texture_creator: &'a TextureCreator<WindowContext>) -> WindowContents<'a> {

		let surface = Surface::load_bmp(path).unwrap();
		WindowContents::Texture(texture_creator.create_texture_from_surface(surface).unwrap())
	}
}

pub struct HierarchalWindow<'a> {
	contents: WindowContents<'a>,
	top_left: Vec2f,
	bottom_right: Vec2f,

	/* TODO:
	- Maybe do some splitting thing here
	- Or include a list of children
	*/

	child: Option<Box<HierarchalWindow<'a>>>
}

impl HierarchalWindow<'_> {
	pub fn new<'a>(
		contents: WindowContents<'a>,
		top_left: Vec2f, bottom_right: Vec2f,
		child: Option<HierarchalWindow<'a>>) -> HierarchalWindow<'a> {

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

	// TODO: catch every error for each match branch
	match &window.contents {
		WindowContents::PlainColor(color) => {
			let use_blending = color.a != 255 && sdl_canvas.blend_mode() != BlendMode::Blend;

			// TODO: make this state transition more efficient
			if use_blending {sdl_canvas.set_blend_mode(BlendMode::Blend);}
				sdl_canvas.set_draw_color(color.clone());
				let _ = sdl_canvas.fill_rect(rescaled_rect);
			if use_blending {sdl_canvas.set_blend_mode(BlendMode::None);}

		},

		WindowContents::Texture(texture) => {
			let _ = sdl_canvas.copy(texture, None, rescaled_rect);
		}
	};

	if let Some(child) = &window.child {
		render_windows_recursively(child, sdl_canvas, rescaled_rect);
	}
}
