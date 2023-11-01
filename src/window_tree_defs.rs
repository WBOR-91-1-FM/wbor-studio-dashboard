use crate::{
	utility_types::{
		update_rate::UpdateRate,
		dynamic_optional::DynamicOptional,
		generic_result::GenericResult, vec2f::Vec2f
	},

	texture::{TexturePool, TextureCreationInfo},
	spinitron::{model::{SpinitronModel, SpinitronModelName}, state::SpinitronState},
	window_tree::{Window, WindowContents, PossibleWindowUpdater, PossibleSharedWindowStateUpdater}
};

struct SharedWindowState {
	spinitron_state: SpinitronState
}

/////////

// This returns a top-level window, shared window state, and a shared window state updater
pub fn make_example_window(texture_pool: &mut TexturePool)
	-> GenericResult<(Window, DynamicOptional, PossibleSharedWindowStateUpdater)> {

	fn update_texture_contents(wc: &mut WindowContents, model: &dyn SpinitronModel,
		texture_pool: &mut TexturePool, should_remake: bool) -> GenericResult<()> {

		if let WindowContents::Texture(texture) = wc {
			if !should_remake {
				println!("Skipping remake");
				return Ok(());
			}

			else if let Some(texture_creation_info) = model.get_texture_creation_info() {
				texture_pool.remake_texture(texture, texture_creation_info)?;
				println!("Remaking a texture in its slot")
			}
			else {
				// TODO: go to a fallback texture here (but use a previously created one though)
				println!("Make a fallback texture");
				*wc = WindowContents::Texture(texture_pool.make_texture(TextureCreationInfo::Path("assets/bird.bmp"))?);
			}
		}
		else {
			if let Some(texture_creation_info) = model.get_texture_creation_info() {
				println!("Making a first-time texture");
				*wc = WindowContents::Texture(texture_pool.make_texture(texture_creation_info)?);
			}
			else {
				println!("Make a first-time fallback texture");
				*wc = WindowContents::Texture(texture_pool.make_texture(TextureCreationInfo::Path("assets/bird.bmp"))?);
			}
		}

		Ok(())
	}

	/* TODO: add the ability to have multiple updaters per window
	(with different update rates). Or, do async requests. */
	fn model_updater(window: &mut Window, texture_pool: &mut TexturePool,
		shared_state: &DynamicOptional) -> GenericResult<()> {

		let inner_shared_state: &SharedWindowState = shared_state.get_inner_value_immut();
		let spinitron_state = &inner_shared_state.spinitron_state;

		let model_name: SpinitronModelName = *window.state.get_inner_value_immut();
		let model = spinitron_state.get_model_by_name(model_name);
		let model_was_updated = inner_shared_state.spinitron_state.model_was_updated(model_name);

		/* TODO: in cases where the `get` request fails here (or in other places), use a fallback texture.
		This happens with Ace Body Movers, with this URL: `https://farm7.staticflickr.com/6179/6172022528_614b745ae8_m.jpg` */
		update_texture_contents(&mut window.contents, model, texture_pool, model_was_updated)
	}

	//////////

	let (individual_update_rate, shared_update_rate) = (
		UpdateRate::new(2.0),
		UpdateRate::new(2.0)
	);

	let model_window_updater: PossibleWindowUpdater = Some((model_updater, individual_update_rate));

	//////////

	let size = Vec2f::new(0.3, 0.4);

	// `tl` = top left
	let spin_tl = Vec2f::new(0.1, 0.1);
	let playlist_tl = spin_tl.translate_x(0.5);
	let persona_tl = spin_tl.translate_y(size.y());
	let show_tl = Vec2f::new(playlist_tl.x(), persona_tl.y());

	//////////

	let spin_window = Window::new(
		model_window_updater,
		DynamicOptional::new(SpinitronModelName::Spin),
		WindowContents::Nothing,
		spin_tl,
		size,
		None
	);

	let playlist_window = Window::new(
		model_window_updater,
		DynamicOptional::new(SpinitronModelName::Playlist),
		WindowContents::Nothing,
		playlist_tl,
		size,
		None
	);

	let persona_window = Window::new(
		model_window_updater,
		DynamicOptional::new(SpinitronModelName::Persona),
		WindowContents::Nothing,
		persona_tl,
		size,
		None
	);

	let show_window = Window::new(
		model_window_updater,
		DynamicOptional::new(SpinitronModelName::Show),
		WindowContents::Nothing,
		show_tl,
		size,
		None
	);

	let top_level_window = Window::new(
		None,
		DynamicOptional::none(),
		WindowContents::make_color(210, 180, 140),
		Vec2f::new(0.01, 0.01),
		Vec2f::new(0.98, 0.98),
		Some(vec![spin_window, playlist_window, persona_window, show_window])
	);

	//////////

	let boxed_shared_state = DynamicOptional::new(
		SharedWindowState {spinitron_state: SpinitronState::new()?}
	);

	fn shared_window_state_updater(state: &mut DynamicOptional) -> GenericResult<()> {
		let state: &mut SharedWindowState = state.get_inner_value_mut();
		state.spinitron_state.update()
	}

	//////////

	Ok((
		top_level_window,
		boxed_shared_state,
		Some((shared_window_state_updater, shared_update_rate))
	))
}
