use sdl2::ttf::{FontStyle, Hinting};

use crate::{
	utility_types::{
		update_rate::UpdateRate,
		dynamic_optional::DynamicOptional,
		generic_result::GenericResult, vec2f::Vec2f
	},

	texture::{TexturePool, TextTextureCreationInfo, TextureCreationInfo},
	spinitron::{model::{SpinitronModelName, MaybeTextureCreationInfo}, state::SpinitronState},
	window_tree::{Window, WindowContents, WindowUpdaterParams, PossibleWindowUpdater, PossibleSharedWindowStateUpdater}
};

struct SharedWindowState {
	spinitron_state: SpinitronState
}

struct IndividualWindowState {
	model_name: SpinitronModelName,
	is_text_window: bool
}

////////// TODO: maybe split `make_wbor_dashboard` into some smaller sub-functions

// This returns a top-level window, shared window state, and a shared window state updater
pub fn make_wbor_dashboard(texture_pool: &mut TexturePool)
	-> GenericResult<(Window, DynamicOptional, PossibleSharedWindowStateUpdater)> {

	fn update_texture_contents(window_contents: &mut WindowContents,
		texture_pool: &mut TexturePool,
		texture_creation_info: &MaybeTextureCreationInfo,
		should_remake: bool) -> GenericResult<()> {

		/* TODO:
		- Instead of making a fallback texture here, just make the window contents `None`, and then use a premade fallback texture as late as possible.
		- Perhaps I should also abstract away the `WindowContents::Texture` usage here?
		- Putting this function in some module for texture operations would be nice too.
		*/
		if let WindowContents::Texture(texture) = window_contents {
			if !should_remake {
				println!("Skipping remake");
				return Ok(());
			}

			if let Some(inner) = texture_creation_info {
				println!("Remaking a texture in its slot");
				texture_pool.remake_texture(texture, inner)?;
			}
			else {
				// TODO: go to a fallback texture here (but use a previously created one though)
				println!("Making a fallback texture");
				*window_contents = WindowContents::Texture(texture_pool.make_texture(&TextureCreationInfo::Path("assets/bird.bmp"))?);
			}
		}
		else {
			if let Some(inner) = texture_creation_info {
				println!("Making a first-time texture");
				*window_contents = WindowContents::Texture(texture_pool.make_texture(inner)?);
			}
			else {
				println!("Making a first-time fallback texture");
				*window_contents = WindowContents::Texture(texture_pool.make_texture(&TextureCreationInfo::Path("assets/bird.bmp"))?);
			}
		}

		Ok(())
	}

	/* TODO: add the ability to have multiple updaters per window
	(with different update rates). Or, do async requests. */
	fn model_updater((window, texture_pool,
		shared_state, area_drawn_to_screen): WindowUpdaterParams
	) -> GenericResult<()> {

		let inner_shared_state: &SharedWindowState = shared_state.get_inner_value_immut();
		let spinitron_state = &inner_shared_state.spinitron_state;

		let individual_window_state: &IndividualWindowState = window.state.get_inner_value_immut();
		let model_name = individual_window_state.model_name;

		let model = spinitron_state.get_model_by_name(model_name);
		let model_was_updated = spinitron_state.model_was_updated(model_name);

		/* TODO: in cases where the `get` request fails here (or in other places), use a fallback texture.
		This happens with 'Ace Body Movers', with this URL: 'https://farm7.staticflickr.com/6179/6172022528_614b745ae8_m.jpg'
		The same thing happens for 'No Things Considered', with this URL: 'https://farm6.staticflickr.com/5085/5254719116_517ee68493_m.jpg'
		Also for 'Controversial Controversy', with this URL: 'https://farm7.staticflickr.com/6089/6150707938_ae60d801be_m.jpg' */

		let wrapped_text_color = WindowContents::make_transparent_color(255, 0, 0, 0.7);
		let text_color = match wrapped_text_color {WindowContents::Color(c) => c, _ => panic!()};

		let text_to_display = format!("{} ", model.to_string());

		// TODO: vary the params based on the text window
		let texture_creation_info = if individual_window_state.is_text_window {
			Some(TextureCreationInfo::Text(TextTextureCreationInfo {
				text_to_display,
				font_path: "assets/ldf_comic_sans.ttf",

				style: FontStyle::ITALIC,
				hinting: Hinting::Normal,
				color: text_color,

				scroll_fn: |secs_since_unix_epoch| {
					// let repeat_rate_secs = 5.0;
					// ((secs_since_unix_epoch % repeat_rate_secs) / repeat_rate_secs, true)

					(secs_since_unix_epoch.sin() * 0.5 + 0.5, false)
				},

				// TODO: why does cutting the max pixel width in half still work?
				max_pixel_width: area_drawn_to_screen.width(),
				pixel_height: area_drawn_to_screen.height()
			}))
		}
		else {
			model.get_texture_creation_info()
		};

		update_texture_contents(
			&mut window.contents, texture_pool,
			&texture_creation_info, model_was_updated
		)?;

		Ok(())

	}

	////////// Making the model windows

	let (individual_update_rate, shared_update_rate) = (
		UpdateRate::new(10.0),
		UpdateRate::new(10.0)
	);

	let model_window_updater: PossibleWindowUpdater = Some((model_updater, individual_update_rate));

	// This cannot exceed 0.5
	let model_window_size = Vec2f::new_from_one(0.4);

	let overspill_amount_to_right = -(model_window_size.x() * 2.0 - 1.0);
	let gap_size = overspill_amount_to_right / 3.0;

	// `tl` = top left

	let spin_tl = Vec2f::new_from_one(gap_size);
	let playlist_tl = spin_tl.translate_x(model_window_size.x() + gap_size);

	let persona_tl = spin_tl.translate_y(model_window_size.y() + gap_size);
	let show_tl = Vec2f::new(playlist_tl.x(), persona_tl.y());

	let (text_tl, text_size) = (Vec2f::new_from_one(0.0), Vec2f::new(1.0, 0.1));

	let model_window_metadata = [
		(SpinitronModelName::Spin, spin_tl),
		(SpinitronModelName::Playlist, playlist_tl),
		(SpinitronModelName::Persona, persona_tl),
		(SpinitronModelName::Show, show_tl)
	];

	let mut all_windows: Vec<Window> = model_window_metadata.iter().map(|metadata| {
		let model_name = metadata.0;

		let text_child = Window::new(
			model_window_updater,

			DynamicOptional::new(IndividualWindowState {
				model_name, is_text_window: true
			}),

			WindowContents::Nothing,
			text_tl,
			text_size,
			None
		);

		return Window::new(
			model_window_updater,

			DynamicOptional::new(IndividualWindowState {
				model_name, is_text_window: false
			}),

			WindowContents::Nothing,
			metadata.1,
			model_window_size,
			Some(vec![text_child])
		)
	}).collect();

	//////////

	// TODO: put more little images in the corners
	let logo_window = Window::new(
		None,
		DynamicOptional::none(),

		WindowContents::Texture(texture_pool.make_texture(
			&TextureCreationInfo::Path("assets/wbor_logo.png")
		)?),

		Vec2f::new(0.0, 0.0),
		Vec2f::new(0.1, 0.05),
		None
	);

	all_windows.insert(0, logo_window);

	//////////

	let top_level_edge_size = 0.025;

	let top_level_window = Window::new(
		None,
		DynamicOptional::none(),
		WindowContents::make_color(210, 180, 140),
		Vec2f::new_from_one(top_level_edge_size),
		Vec2f::new_from_one(1.0 - top_level_edge_size * 2.0),
		Some(all_windows)
	);

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
