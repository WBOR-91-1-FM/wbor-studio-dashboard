use sdl2::ttf::{FontStyle, Hinting};

use crate::{
	utility_types::{
		update_rate::UpdateRate,
		dynamic_optional::DynamicOptional,
		generic_result::GenericResult, vec2f::Vec2f
	},

	texture::{TexturePool, TextTextureCreationInfo, TextureCreationInfo},
	spinitron::{model::SpinitronModelName, state::SpinitronState},
	window_tree::{Window, WindowContents, WindowUpdaterParams, PossibleWindowUpdater, PossibleSharedWindowStateUpdater}
};

struct SharedWindowState<'a> {
	spinitron_state: SpinitronState,

	// This is used whenever a texture can't be loaded
	fallback_texture_creation_info: TextureCreationInfo<'a>
}

struct IndividualWindowState {
	model_name: SpinitronModelName,
	is_text_window: bool
}

////////// TODO: maybe split `make_wbor_dashboard` into some smaller sub-functions

/* TODO:
- Rename all `Possible` types to `Maybe`s (incl. the associated variable names) (and all `inner-prefixed` vars too)
- Run `clippy`
*/

// TODO: should this be a member function for `Window`?
fn update_window_texture_contents(
	should_remake: bool,
	window_contents: &mut WindowContents,
	texture_pool: &mut TexturePool,
	texture_creation_info: &TextureCreationInfo,
	fallback_texture_creation_info: &TextureCreationInfo) -> GenericResult<()> {

	/* This is a macro for making or remaking a texture. If making or
	remaking fails, a fallback texture is put into that texture's slot. */
	macro_rules! try_to_make_or_remake_texture {
		($make_or_remake: expr, $make_or_remake_description: expr, $($extra_args:expr),*) => {{
			$make_or_remake(texture_creation_info, $($extra_args),*).or_else(
				|failure_reason| {
					println!("Unexpectedly failed while trying to {} texture, and reverting to a fallback \
						texture. Reason: '{}'.", $make_or_remake_description, failure_reason);

					$make_or_remake(fallback_texture_creation_info, $($extra_args),*)
				}
			)
		}};
	}

	let updated_texture = if let WindowContents::Texture(prev_texture) = window_contents {
		if should_remake {try_to_make_or_remake_texture!(|a, b| texture_pool.remake_texture(a, b), "remake an existing", prev_texture)?}
		prev_texture.clone()
	}
	else {
		/* There was not a texture before, and there's an initial one available now,
		so a first texture is being made. This should only happen once, at the program's
		start; otherwise, an unbound amount of new textures will be made. */
		try_to_make_or_remake_texture!(|a| texture_pool.make_texture(a), "make a new",)?
	};

	*window_contents = WindowContents::Texture(updated_texture);
	Ok(())
}

// This returns a top-level window, shared window state, and a shared window state updater
pub fn make_wbor_dashboard(texture_pool: &mut TexturePool)
	-> GenericResult<(Window, DynamicOptional, PossibleSharedWindowStateUpdater)> {

	/* TODO: add the ability to have multiple updaters per window
	(with different update rates). Or, do async requests. */
	fn model_updater((window, texture_pool,
		shared_state, area_drawn_to_screen): WindowUpdaterParams) -> GenericResult<()> {

		let inner_shared_state: &SharedWindowState = shared_state.get_inner_value();
		let spinitron_state = &inner_shared_state.spinitron_state;

		let individual_window_state: &IndividualWindowState = window.get_state();
		let model_name = individual_window_state.model_name;

		let model = spinitron_state.get_model_by_name(model_name);
		let model_was_updated = spinitron_state.model_was_updated(model_name);

		let wrapped_text_color = WindowContents::make_transparent_color(255, 0, 0, 0.7);
		let text_color = match wrapped_text_color {WindowContents::Color(c) => c, _ => panic!()};

		let text_to_display = format!("{} ", model.to_string());

		// TODO: vary the params based on the text window
		let texture_creation_info = if individual_window_state.is_text_window {
			TextureCreationInfo::Text(TextTextureCreationInfo {
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
			})
		}
		else {
			match model.get_texture_creation_info() {
				Some(texture_creation_info) => texture_creation_info,
				None => inner_shared_state.fallback_texture_creation_info.clone()
			}
		};

		update_window_texture_contents(
			model_was_updated,
			window.get_contents_mut(),
			texture_pool,
			&texture_creation_info,
			&inner_shared_state.fallback_texture_creation_info
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
		SharedWindowState {
			spinitron_state: SpinitronState::new()?,
			fallback_texture_creation_info: TextureCreationInfo::Path("assets/wbor_no_texture_available.png")
		}
	);

	fn shared_window_state_updater(state: &mut DynamicOptional) -> GenericResult<()> {
		let state: &mut SharedWindowState = state.get_inner_value_mut();
		state.spinitron_state.update()
	}

	//////////

	// TODO: past a certain point, make sure that the texture pool never grows

	Ok((
		top_level_window,
		boxed_shared_state,
		Some((shared_window_state_updater, shared_update_rate))
	))
}
