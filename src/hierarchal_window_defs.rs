use sdl2;

use crate::spinitron;
use crate::vec2f::Vec2f;
use crate::texture::TexturePool;
use crate::generic_result::GenericResult;
use crate::window_hierarchy::{WindowContents, HierarchalWindow};

pub fn make_example_window(texture_creator: &sdl2::render::TextureCreator<sdl2::video::WindowContext>)
	-> GenericResult<(HierarchalWindow, TexturePool)> {

	let api_key = spinitron::ApiKey::new()?;
	let mut texture_pool = TexturePool::new(texture_creator);

	let (spin, playlist, persona, show) = spinitron::get_current_data(&api_key)?;
	let fallback_contents = WindowContents::Texture(texture_pool.make_texture_from_path("assets/wbor_plane.bmp")?);
	let current_album_contents = spinitron::get_current_album_contents(&spin, &mut texture_pool, fallback_contents)?;

	/*
	println!("Spin: {:?}\n", spin);
	println!("Playlist: {:?}\n", playlist);
	println!("Persona: {:?}\n", persona);
	println!("Show: {:?}\n", show);
	*/

	let album_cover = HierarchalWindow::new(
		None,
		None,
		current_album_contents,
		Vec2f::new(0.4, 0.1),
		Vec2f::new(0.7, 0.9),
		None
	);

	let bird = HierarchalWindow::new(
		None,
		None,
		WindowContents::Texture(texture_pool.make_texture_from_path("assets/bird.bmp")?),
		Vec2f::new(0.1, 0.1),
		Vec2f::new(0.3, 0.9),
		None
	);

	let photo_box = HierarchalWindow::new(
		None,
		None,
		WindowContents::make_transparent_color(0, 255, 0, 0.8),
		Vec2f::new(0.01, 0.01),
		Vec2f::new(0.75, 0.5),
		Some(vec![album_cover, bird])
	);

	let example_window = HierarchalWindow::new(
		None,
		None,
		WindowContents::make_color(255, 0, 0),
		Vec2f::new(0.01, 0.01),
		Vec2f::new(0.99, 0.99),
		Some(vec![photo_box])
	);

	Ok((example_window, texture_pool))
}
