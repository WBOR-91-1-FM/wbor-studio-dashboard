use sdl2;

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
		println!("{:?}", updated_state);

		// TODO: set a flag for when the current spin or playlist changed, so that child windows can newly render those changes

		Ok(())
	}

	let beige_ish_tan = WindowContents::make_color(210, 180, 140);

	let fps = 60;
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
