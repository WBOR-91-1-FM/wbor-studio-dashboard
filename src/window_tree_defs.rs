use sdl2;

use crate::spinitron;
use crate::vec2f::Vec2f;
use crate::dynamic_optional;
use crate::texture::TexturePool;
use crate::generic_result::GenericResult;
use crate::window_tree::{WindowContents, Window};

pub fn make_example_window(texture_creator: &sdl2::render::TextureCreator<sdl2::video::WindowContext>)
	-> GenericResult<(Window, TexturePool)> {

	let api_key = spinitron::ApiKey::new()?;
	let mut texture_pool = TexturePool::new(texture_creator);

	// TODO: if there is no current spin, will this only return the last one?
	let spin = spinitron::get_current_spin(&api_key)?;
	let fallback_contents = WindowContents::Texture(texture_pool.make_texture_from_path("assets/wbor_plane.bmp")?);
	let current_album_contents = spinitron::get_current_album_contents(&spin, &mut texture_pool, fallback_contents)?;

	struct WindowState {
		api_key: spinitron::ApiKey,
		spin: spinitron::Spin
	}

	let window_state: dynamic_optional::DynamicOptional = Some(Box::new(WindowState {
		api_key, spin
	}));

	fn example_window_updater(window: &mut Window, texture_pool: &mut TexturePool) -> GenericResult<()> {
		let generic_state = &mut window.state;

		if generic_state.is_some() {
			let state: &mut WindowState = dynamic_optional::get_inner_value(generic_state);

			let current_spin = spinitron::get_current_spin(&state.api_key)?;

			if current_spin.id == state.spin.id {
				println!("Current spin is unchanged");
			}
			else {
				println!("There's a new spin, {:?}, so replace the old one", current_spin);
				state.spin = current_spin;
			}

			println!("---");
		}

		/* Hm - how can I then pass this state down to child windows?
		Maybe share some external state via a `Rc`? */

		/*
		if let Some(children) = &mut window.children {
			for child in children {
				example_window_updater(child, texture_pool)?
			}
		}
		*/

		Ok(())
	}

	let example_window_update_rate = 200;

	let album_cover = Window::new(
		None,
		None,
		current_album_contents,
		Vec2f::new(0.4, 0.1),
		Vec2f::new(0.7, 0.9),
		None
	);

	let bird = Window::new(
		None,
		None,
		WindowContents::Texture(texture_pool.make_texture_from_path("assets/bird.bmp")?),
		Vec2f::new(0.1, 0.1),
		Vec2f::new(0.3, 0.9),
		None
	);

	let photo_box = Window::new(
		None,
		None,
		WindowContents::make_transparent_color(0, 255, 0, 0.8),
		Vec2f::new(0.01, 0.01),
		Vec2f::new(0.75, 0.5),
		Some(vec![album_cover, bird])
	);

	let example_window = Window::new(
		Some((example_window_updater, example_window_update_rate)),
		window_state,
		WindowContents::make_color(255, 0, 0),
		Vec2f::new(0.01, 0.01),
		Vec2f::new(0.99, 0.99),
		Some(vec![photo_box])
	);

	Ok((example_window, texture_pool))
}
