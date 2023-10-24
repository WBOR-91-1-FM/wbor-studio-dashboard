use sdl2;

use crate::spinitron;
use crate::texture::TexturePool;

use crate::utility_types::{
	dynamic_optional, generic_result::GenericResult, vec2f::Vec2f
};

use crate::spinitron::state::SpinitronState;
use crate::window_tree::{WindowContents, Window};

pub fn make_example_window(texture_creator: &sdl2::render::TextureCreator<sdl2::video::WindowContext>)
	-> GenericResult<(Window, TexturePool)> {

	// TODO: eventually, make an updater for a global shared state, for all of the windows (`SpinitronState` will be there).
	fn top_level_window_updater(window: &mut Window, texture_pool: &mut TexturePool) -> GenericResult<()> {
		let state: &mut SpinitronState = dynamic_optional::get_inner_value(&mut window.state);
		let updated_state = state.update()?; // TODO: store this, so that child windows know when to rebuild their contents

		if updated_state.0 {println!("New spin: {:?}", state.get_spin());}
		if updated_state.1 {println!("New playlist: {:?}", state.get_playlist());}
		if updated_state.2 {println!("New persona: {:?}", state.get_persona());}
		if updated_state.3 {println!("New show: {:?}", state.get_show());}

		if updated_state.0 || updated_state.1 || updated_state.2 || updated_state.3 {
			println!("\n\n\n---\n\n");
		}

		let updated_spin = updated_state.0;
		let window_contents_not_texture_yet = if let WindowContents::Texture(_) = window.contents {false} else {true};

		if updated_spin || window_contents_not_texture_yet {
			/* TODO:
			- Use the old texture slot when doing this (otherwise I will run out of memory),
			- If the URL is the same as the previous one, don't reload
			*/

			/*
			let maybe_texture = spinitron::api::get_texture_from_optional_url(&state.get_spin().get_image_link(), texture_pool);

			if let Some(texture) = maybe_texture {
				window.contents = texture?;
			}
			else {
				// TODO: otherwise, set a fallback texture
			}
			*/
		}

		Ok(())
	}

	let beige_ish_tan = WindowContents::make_color(210, 180, 140);

	let fps = 60; // TODO: don't hardcode this
	let update_rate_in_secs = 5;
	let top_level_window_update_rate = fps * update_rate_in_secs;

	let spinitron_state = SpinitronState::new()?;
	let boxed_spinitron_state: dynamic_optional::DynamicOptional = Some(Box::new(spinitron_state));
	let texture_pool = TexturePool::new(texture_creator);

	let album_cover_window = Window::new(
		None,
		None,
		WindowContents::Nothing,
		Vec2f::new(0.4, 0.1),
		Vec2f::new(0.7, 0.9),
		None
	);

	let top_level_window = Window::new(
		Some((top_level_window_updater, top_level_window_update_rate)),
		boxed_spinitron_state,
		beige_ish_tan,
		Vec2f::new(0.01, 0.01),
		Vec2f::new(0.99, 0.99),
		Some(vec![album_cover_window])
	);

	Ok((top_level_window, texture_pool))
}
