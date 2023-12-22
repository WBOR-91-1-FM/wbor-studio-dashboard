use chrono::{Local, Timelike};

use sdl2::ttf::{FontStyle, Hinting};

use crate::{
	utility_types::{
		update_rate::UpdateRate,
		dynamic_optional::DynamicOptional,
		generic_result::GenericResult, vec2f::Vec2f
	},

	texture::{TexturePool, FontInfo, TextDisplayInfo, TextureCreationInfo},
	spinitron::{model::SpinitronModelName, state::SpinitronState},

	window_tree::{
		GeneralLine, Window,
		WindowContents, WindowUpdaterParams,
		PossibleWindowUpdater, PossibleSharedWindowStateUpdater, ColorSDL
	}
};

// TODO: put the clock stuff in its own file

// This is called raw because it's centered at (0, 0) and is unrotated.
type RawClockHand = GeneralLine<(f32, f32)>;

const NUM_CLOCK_HANDS: usize = 4;
const CLOCK_CENTER: (f32, f32) = (0.5, 0.5);

struct SharedWindowState<'a> {
	raw_clock_hands: [RawClockHand; NUM_CLOCK_HANDS],
	spinitron_state: SpinitronState,

	// This is used whenever a texture can't be loaded
	fallback_texture_creation_info: TextureCreationInfo<'a>
}

struct SpinitronModelWindowState {
	model_name: SpinitronModelName,
	is_text_window: bool
}

//////////

// These extents are defined assuming that the clock is pointing to 12:00
struct ClockHandConfig {
	x_extent: f32,
	minor_y_extent: f32,
	major_y_extent: f32,
	color: ColorSDL
}

impl ClockHandConfig {
	fn make_geometry(&self) -> RawClockHand {
		let hand = [
			// The minor part of the hand
			(-self.x_extent, 0.0),
			(0.0, self.minor_y_extent),
			(self.x_extent, 0.0),

			// The major part of the hand
			(0.0, -self.major_y_extent),
			(-self.x_extent, 0.0)
		];

		(self.color, hand.to_vec())
	}
}

////////// TODO: maybe split `make_wbor_dashboard` into some smaller sub-functions

/* TODO:
- Rename all `Possible` types to `Maybe`s (incl. the associated variable names) (and all `inner-prefixed` vars too)
- Run `clippy`
*/

fn make_clock_window_and_raw_hands(
	update_rate: UpdateRate,
	top_left: Vec2f,
	size: Vec2f,
	ms_sec_min_hour_hand_configs: [ClockHandConfig; NUM_CLOCK_HANDS],
	dial_texture_path: &str,
	texture_pool: &mut TexturePool) -> GenericResult<(Window, [RawClockHand; NUM_CLOCK_HANDS])> {

	fn updater_fn((window, _, shared_window_state, _): WindowUpdaterParams) -> GenericResult<()> {
		let curr_time = Local::now();

		let time_units: [(u32, u32); NUM_CLOCK_HANDS] = [
			(curr_time.timestamp_subsec_millis(), 1000),
			(curr_time.second(), 60),
			(curr_time.minute(), 60),
			(curr_time.hour() % 12, 12)
		];

		let inner_shared_window_state: &SharedWindowState = shared_window_state.get_inner_value();
		let raw_clock_hands = &inner_shared_window_state.raw_clock_hands;
		let WindowContents::Lines(rotated_hands) = window.get_contents_mut() else {panic!()};

		let mut prev_time_fract = 0.0;

		for (i, time_unit) in time_units.iter().enumerate() {
			let time_fract = (time_unit.0 as f32 + prev_time_fract) / time_unit.1 as f32;
			prev_time_fract = time_fract;

			let angle = time_fract * std::f32::consts::TAU;
			let (cos_angle, sin_angle) = (angle.cos(), angle.sin());

			let raw_hand = &raw_clock_hands[i];
			let rotated_hand = &mut rotated_hands[(NUM_CLOCK_HANDS - 1) - i].1;

			rotated_hand.iter_mut().enumerate().for_each(|(index, dest)| {
				let raw = raw_hand.1[index];

				*dest = Vec2f::new(
					(raw.0 * cos_angle - raw.1 * sin_angle) + CLOCK_CENTER.0,
					(raw.0 * sin_angle + raw.1 * cos_angle) + CLOCK_CENTER.1
				);
			});
		}

		Ok(())
	}

	let raw_clock_hands = ms_sec_min_hour_hand_configs.map(|config| config.make_geometry());

	let clock_window = Window::new(
		Some((updater_fn, update_rate)),
		DynamicOptional::none(),

		WindowContents::Lines(
			raw_clock_hands.iter().rev().map(|(color, clock_hand)| {
				(*color, vec![Vec2f::new_from_one(0.0); clock_hand.len()])
			}).collect()
		),

		None,
		Vec2f::new_from_one(0.0),
		Vec2f::new_from_one(1.0),
		None
	);

	Ok((Window::new(
		None,
		DynamicOptional::none(),

		WindowContents::Texture(texture_pool.make_texture(
			&TextureCreationInfo::Path(dial_texture_path)
		)?),

		None,
		top_left,
		size,
		Some(vec![clock_window])
	), raw_clock_hands))
}

// This returns a top-level window, shared window state, and a shared window state updater
pub fn make_wbor_dashboard(texture_pool: &mut TexturePool)
	-> GenericResult<(Window, DynamicOptional, PossibleSharedWindowStateUpdater)> {

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

		window.update_texture_contents(
			model_was_updated,
			texture_pool,
			&texture_creation_info,
			&inner_shared_state.fallback_texture_creation_info
		)
	}

	////////// Making the model windows

	let (individual_update_rate, shared_update_rate) = (
		UpdateRate::new(10.0),
		UpdateRate::new(10.0)
	);

	let spinitron_model_window_updater: PossibleWindowUpdater = Some((spinitron_model_window_updater_fn, individual_update_rate));

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

	let spinitron_model_window_metadata = [
		(SpinitronModelName::Spin, spin_tl),
		(SpinitronModelName::Playlist, playlist_tl),
		(SpinitronModelName::Persona, persona_tl),
		(SpinitronModelName::Show, show_tl)
	];

	let mut all_windows: Vec<Window> = spinitron_model_window_metadata.iter().map(|metadata| {
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
	}).collect();

	////////// Making a logo window

	// TODO: put more little images in the corners
	let logo_window = Window::new(
		None,
		DynamicOptional::none(),

		WindowContents::Texture(texture_pool.make_texture(
			&TextureCreationInfo::Path("assets/wbor_logo.png")
		)?),

		None,

		Vec2f::new_from_one(0.0),
		Vec2f::new(0.1, 0.05),
		None
	);

	all_windows.push(logo_window);

	////////// Making a clock window

	let (clock_window, raw_clock_hands) = make_clock_window_and_raw_hands(
		UpdateRate::new(1.0 / 60.0),

		Vec2f::new(1.0 - gap_size, 0.0),
		Vec2f::new_from_one(gap_size),

		[
			ClockHandConfig {minor_y_extent: 0.2, major_y_extent: 0.5, x_extent: 0.01, color: ColorSDL::RGBA(255, 0, 0, 100)}, // Milliseconds
			ClockHandConfig {minor_y_extent: 0.02, major_y_extent: 0.48, x_extent: 0.01, color: ColorSDL::WHITE}, // Seconds
			ClockHandConfig {minor_y_extent: 0.02, major_y_extent: 0.35, x_extent: 0.01, color: ColorSDL::YELLOW}, // Minutes
			ClockHandConfig {minor_y_extent: 0.02, major_y_extent: 0.2, x_extent: 0.01, color: ColorSDL::BLACK} // Hours
		],

		"assets/wbor_watch_dial.png",
		texture_pool
	)?;

	all_windows.push(clock_window);

	//////////

	let top_level_edge_size = 0.025;

	let top_level_window = Window::new(
		None,
		DynamicOptional::none(),
		WindowContents::Color(ColorSDL::RGB(210, 180, 140)),
		None,
		Vec2f::new_from_one(top_level_edge_size),
		Vec2f::new_from_one(1.0 - top_level_edge_size * 2.0),
		Some(all_windows)
	);

	let boxed_shared_state = DynamicOptional::new(
		SharedWindowState {
			raw_clock_hands,
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
		top_level_window,
		boxed_shared_state,
		Some((shared_window_state_updater, shared_update_rate))
	))
}
