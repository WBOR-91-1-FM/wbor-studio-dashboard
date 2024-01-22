use sdl2::ttf::{FontStyle, Hinting};

use crate::{
	window_tree_defs::clock::{ClockHandConfig, ClockHandConfigs, ClockHands},

	utility_types::{
		update_rate::UpdateRate,
		dynamic_optional::DynamicOptional,
		generic_result::GenericResult, vec2f::Vec2f
	},

	texture::{TexturePool, FontInfo, TextDisplayInfo, TextureCreationInfo},
	spinitron::{model::SpinitronModelName, state::SpinitronState},

	window_tree::{
		Window,
		WindowContents, WindowUpdaterParams,
		PossibleWindowUpdater, PossibleSharedWindowStateUpdater, ColorSDL
	},

	window_tree_defs::shared_window_state::SharedWindowState
};

struct SpinitronModelWindowState {
	model_name: SpinitronModelName,
	is_text_window: bool
}

////////// TODO: maybe split `make_wbor_dashboard` into some smaller sub-functions

/* TODO:
- Rename all `Possible` types to `Maybe`s (incl. the associated variable names) (and all `inner-prefixed` vars too)
- Run `clippy`
*/

fn make_spinitron_windows(
	model_window_size: Vec2f, gap_size: f32,
	model_update_rate: UpdateRate) -> Vec<Window> {

	const FONT_INFO: FontInfo = FontInfo {
		path: "assets/fonts/Gohu/GohuFontuni14NerdFont-Regular.ttf",
		style: FontStyle::ITALIC,
		hinting: Hinting::Normal
	};

	/* TODO: add the ability to have multiple updaters per window
	(with different update rates). Or, do async requests. */
	fn spinitron_model_window_updater_fn((window, texture_pool,
		shared_state, area_drawn_to_screen): WindowUpdaterParams) -> GenericResult<()> {

		let inner_shared_state: &SharedWindowState = shared_state.get_inner_value();
		let spinitron_state = &inner_shared_state.spinitron_state;

		let individual_window_state: &SpinitronModelWindowState = window.get_state();
		let model_name = individual_window_state.model_name;

		let model = spinitron_state.get_model_by_name(model_name);
		let model_was_updated = spinitron_state.model_was_updated(model_name);

		let text_color = ColorSDL::RGBA(255, 0, 0, 178);
		let text_to_display = format!("{} ", model.to_string());

		// TODO: vary the params based on the text window
		let texture_creation_info = if individual_window_state.is_text_window {
			TextureCreationInfo::Text((
				&FONT_INFO,

				TextDisplayInfo {
					text: text_to_display,
					color: text_color,

					// TODO: pass in the scroll fn too
					scroll_fn: |secs_since_unix_epoch| {
						// let repeat_rate_secs = 5.0;
						// ((secs_since_unix_epoch % repeat_rate_secs) / repeat_rate_secs, true)

						(secs_since_unix_epoch.sin() * 0.5 + 0.5, false)
					},

					// TODO: why does cutting the max pixel width in half still work?
					max_pixel_width: area_drawn_to_screen.width(),
					pixel_height: area_drawn_to_screen.height()
				}
			))
		}
		else {
			match model.get_texture_creation_info() {
				Some(texture_creation_info) => texture_creation_info,
				None => inner_shared_state.fallback_texture_creation_info.clone()
			}
		};

		// TODO: see if threading will be needed for updating textures as well
		window.update_texture_contents(
			model_was_updated,
			texture_pool,
			&texture_creation_info,
			&inner_shared_state.fallback_texture_creation_info
		)
	}

	////////// Making the model windows

	let spinitron_model_window_updater: PossibleWindowUpdater = Some((spinitron_model_window_updater_fn, model_update_rate));

	// `tl` = top left

	let spin_tl = Vec2f::new_from_one(gap_size);
	let playlist_tl = spin_tl.translate_x(model_window_size.x() + gap_size);

	let persona_tl = spin_tl.translate_y(model_window_size.y() + gap_size);
	let show_tl = Vec2f::new(playlist_tl.x(), persona_tl.y());

	let (text_tl, text_size) = (Vec2f::ZERO, Vec2f::new(1.0, 0.1));

	let spinitron_model_window_metadata = [
		(SpinitronModelName::Spin, spin_tl),
		(SpinitronModelName::Playlist, playlist_tl),
		(SpinitronModelName::Persona, persona_tl),
		(SpinitronModelName::Show, show_tl)
	];

	spinitron_model_window_metadata.iter().map(|metadata| {
		let model_name = metadata.0;

		let text_child = Window::new(
			spinitron_model_window_updater,

			DynamicOptional::new(SpinitronModelWindowState {
				model_name, is_text_window: true
			}),

			WindowContents::Nothing,
			Some(ColorSDL::GREEN),
			text_tl,
			text_size,
			None
		);

		Window::new(
			spinitron_model_window_updater,

			DynamicOptional::new(SpinitronModelWindowState {
				model_name, is_text_window: false
			}),

			WindowContents::Nothing,
			Some(ColorSDL::BLUE),
			metadata.1,
			model_window_size,
			Some(vec![text_child])
		)
	}).collect()
}

// This returns a top-level window, shared window state, and a shared window state updater
pub fn make_wbor_dashboard(texture_pool: &mut TexturePool)
	-> GenericResult<(Window, DynamicOptional, PossibleSharedWindowStateUpdater)> {

	////////// Making the Spinitron windows

	let (individual_update_rate, shared_update_rate) = (
		UpdateRate::new(1.0),
		UpdateRate::new(1.0)
	);

	// This cannot exceed 0.5
	let model_window_size = Vec2f::new_from_one(0.4);

	let overspill_amount_to_right = -(model_window_size.x() * 2.0 - 1.0);
	let model_gap_size = overspill_amount_to_right / 3.0;

	let mut all_windows = make_spinitron_windows(
		model_window_size, model_gap_size,
		individual_update_rate
	);

	// TODO: make a temporary error window that pops up when needed

	////////// Making some static texture windows

	// TODO: make animated textures possible
	// TODO: remove a bunch of async TODOs, and just old ones in general

	let soup_height = model_gap_size * 1.5;

	// Updater, state, texture path, top left, size
	let static_texture_info = [
		(
			"assets/wbor_logo.png",
			Vec2f::ZERO,
			Vec2f::new(0.1, 0.05)
		),

		(
			"assets/wbor_soup.png",
			Vec2f::new(0.0, 1.0 - soup_height),
			Vec2f::new(model_gap_size, soup_height)
		)
	];

	all_windows.extend(static_texture_info.into_iter().map(|datum| {
		return Window::new(
			None,
			DynamicOptional::NONE,

			WindowContents::Texture(texture_pool.make_texture(
				&TextureCreationInfo::Path(datum.0),
			).unwrap()),

			None,

			datum.1,
			datum.2,
			None
		)
	}));

	////////// Making a clock window

	let (clock_hands, clock_window) = ClockHands::new_with_window(
		UpdateRate::ONCE_PER_FRAME,

		// Vec2f::new(1.0 - model_gap_size, 0.0),
		// Vec2f::new_from_one(model_gap_size),

		Vec2f::ZERO,
		Vec2f::ONE,

		ClockHandConfigs {
			milliseconds: ClockHandConfig::new(0.01, 0.2, 0.5, ColorSDL::RGBA(255, 0, 0, 100)), // Milliseconds
			seconds: ClockHandConfig::new(0.01, 0.02, 0.48, ColorSDL::WHITE), // Seconds
			minutes: ClockHandConfig::new(0.01, 0.02, 0.35, ColorSDL::YELLOW), // Minutes
			hours: ClockHandConfig::new(0.01, 0.02, 0.2, ColorSDL::BLACK) // Hours
		},

		"assets/wbor_watch_dial.png",
		texture_pool
	)?;

	all_windows.push(clock_window);

	//////////

	// let top_level_edge_size = 0.025;

	let small_edge_size = 0.015;

	let main_window = Window::new(
		None,
		DynamicOptional::NONE,
		WindowContents::Color(ColorSDL::RGB(210, 180, 140)),
		None,
		Vec2f::new(small_edge_size, 0.08),
		Vec2f::new(1.0 - small_edge_size * 2.0, 0.9),
		// Vec2f::new_from_one(top_level_edge_size),
		// Vec2f::new_from_one(1.0 - top_level_edge_size * 2.0),
		Some(all_windows)
	);

	let boxed_shared_state = DynamicOptional::new(
		SharedWindowState {
			clock_hands,
			spinitron_state: SpinitronState::new()?,
			fallback_texture_creation_info: TextureCreationInfo::Path("assets/wbor_no_texture_available.png"),
		}
	);

	fn shared_window_state_updater(state: &mut DynamicOptional) -> GenericResult<()> {
		let state: &mut SharedWindowState = state.get_inner_value_mut();
		state.spinitron_state.update()
	}

	//////////

	// TODO: past a certain point, make sure that the texture pool never grows

	Ok((
		main_window,
		boxed_shared_state,
		Some((shared_window_state_updater, shared_update_rate))
	))
}
