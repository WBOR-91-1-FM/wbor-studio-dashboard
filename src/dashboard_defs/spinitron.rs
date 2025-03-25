use std::borrow::Cow;

use rand::Rng;

use crate::{
	dashboard_defs::{
		easing_fns,
		funky_remake_transitions,
		shared_window_state::SharedWindowState
	},

	texture::{
		pool::TextureCreationInfo,
		text::{DisplayText, TextDisplayInfo}
	},

	utility_types::{
		time::*,
		vec2f::Vec2f,
		api_history_list,
		generic_result::*,
		update_rate::UpdateRate,
		dynamic_optional::DynamicOptional
	},

	window_tree::{
		Window,
		ColorSDL,
		WindowContents,
		WindowUpdaters,
		WindowUpdaterParams
	},

	spinitron::model::{SpinitronModelName, NUM_SPINITRON_MODEL_TYPES}
};

struct SpinitronModelWindowState {
	model_name: SpinitronModelName,
	maybe_text_color: Option<ColorSDL> // If this is `None`, it is not a text window
}

pub struct SpinitronModelWindowInfo {
	pub tl: Vec2f,
	pub size: Vec2f,
	pub border_color: Option<ColorSDL>
}

pub struct SpinitronModelWindowsInfo {
	pub model_name: SpinitronModelName,
	pub texture_window: Option<SpinitronModelWindowInfo>,
	pub text_window: Option<SpinitronModelWindowInfo>,
	pub text_color: ColorSDL
}

//////////

pub fn make_spinitron_windows(
	all_model_windows_info: &[SpinitronModelWindowsInfo; NUM_SPINITRON_MODEL_TYPES],
	model_update_rate: UpdateRate,

	history_tl: Vec2f, history_size: Vec2f,
	history_border_color: Option<ColorSDL>,
	num_spins_shown_in_history: usize,
	rand_generator: &mut rand::rngs::ThreadRng)

	-> Vec<Window> {

	/* Note: the drawn size passed into this does not account for aspect ratio correction.
	For Spinitron models, the size is only needed for spin textures all and text textures.
	For spin textures, the you can figure out the corrected texture size since spin textures
	are square, and you can get the corrected size by taking the min of the two components
	of `area_drawn_to_screen`. This is done within the `update` function, though! */
	fn spinitron_model_window_updater_fn(params: WindowUpdaterParams) -> MaybeError {
		let inner_shared_state = params.shared_window_state.get_mut::<SharedWindowState>();
		let individual_window_state = params.window.get_state::<SpinitronModelWindowState>();
		let model_name = individual_window_state.model_name;
		let window_size_pixels = params.area_drawn_to_screen;
		let spinitron_state = &mut inner_shared_state.spinitron_state;

		////////// The spin texture window is the window designated for updating the Spinitron state

		let is_text_window = individual_window_state.maybe_text_color.is_some();

		if model_name == SpinitronModelName::Spin && !is_text_window {
			spinitron_state.update(params.area_drawn_to_screen, params.texture_pool, &mut inner_shared_state.error_state)?;
		}

		////////// Checking whether the model's texture should be updated

		let do_model_image_texture_update = !is_text_window && spinitron_state.model_texture_was_updated(model_name);
		let do_model_text_texture_update = is_text_window && spinitron_state.model_text_was_updated(model_name);

		// Doing the `WindowContents::Nothing` match for an initial match during launch
		let do_any_update =
			do_model_image_texture_update ||
			do_model_text_texture_update ||
			matches!(params.window.get_contents(), WindowContents::Nothing);

		if !do_any_update {
			return Ok(());
		}

		////////// Updating the model's texture

		let texture_creation_info = if is_text_window {
			let model_text = spinitron_state.get_cached_model_text(model_name);

			TextureCreationInfo::Text((
				Cow::Borrowed(inner_shared_state.font_info),

				TextDisplayInfo::new(
					DisplayText::new(model_text),
					individual_window_state.maybe_text_color.unwrap(),
					window_size_pixels, // TODO: why does cutting the max pixel width in half still work?

					/* TODO:
					- Pass this in
					- Why doesn't this scroll when the text is short enough? Good, but not programmed in...
					*/
					easing_fns::scroll::OSCILLATE_NO_WRAP,
					2.0
				)
			))
		}
		else {
			spinitron_state.get_cached_texture_creation_info(model_name)
		};

		//////////

		let default_duration = Duration::seconds(3);

		let intermediate_info = funky_remake_transitions::IntermediateTextureTransitionInfo {
			percent_chance_to_show_rand_intermediate_texture: 0.2,
			rand_duration_range_for_intermediate: (0.6, 2.2),
			max_random_transitions: 10
		};

		//////////

		funky_remake_transitions::update_as_texture_with_funky_remake_transition(
			params.window.get_contents_mut(),
			params.texture_pool,
			&texture_creation_info,
			default_duration,
			&mut inner_shared_state.rand_generator,
			inner_shared_state.get_fallback_texture_creation_info,
			intermediate_info
		)
	}

	////////// Making the model windows

	let spinitron_model_window_updaters: WindowUpdaters = vec![(spinitron_model_window_updater_fn, model_update_rate)];

	// TODO: perhaps for making multiple model windows, allow for an option to have sub-model-windows
	let mut spinitron_windows: Vec<Window> = all_model_windows_info.iter().flat_map(|general_info| {
		let mut output_windows = Vec::new();

		let mut maybe_make_model_window =
			|maybe_info: &Option<SpinitronModelWindowInfo>, maybe_text_color: Option<ColorSDL>| {

			if let Some(info) = maybe_info {
				output_windows.push(Window::new(
					spinitron_model_window_updaters.clone(),

					DynamicOptional::new(SpinitronModelWindowState {
						model_name: general_info.model_name,
						maybe_text_color
					}),

					WindowContents::Nothing,
					info.border_color,
					info.tl,
					info.size,
					vec![]
				));
			}
		};

		maybe_make_model_window(&general_info.texture_window, None);
		maybe_make_model_window(&general_info.text_window, Some(general_info.text_color));

		output_windows
	}).collect();

	////////// Building the API history list window

	let sub_width = 1.0 / num_spins_shown_in_history as f64;
	let (jitter_x, jitter_y) = (0.1 * sub_width, 0.3); // TODO: allow for overlap
	let subwindow_size = Vec2f::new(sub_width - jitter_x, 1.0 - jitter_y);

	fn spin_history_item_updater_fn(params: WindowUpdaterParams) -> MaybeError {
		let index = *params.window.get_state::<usize>();
		let inner_shared_state = params.shared_window_state.get_mut::<SharedWindowState>();

		*params.window.get_contents_mut() = if let Some(handle) = inner_shared_state.spinitron_state.get_historic_spin_at_index(
			index, params.area_drawn_to_screen
		) {
			WindowContents::Texture(handle)
		}
		else {
			WindowContents::Nothing
		};

		Ok(())
	}

	spinitron_windows.push(api_history_list::make_api_history_list_window(
		history_tl,
		history_size,
		subwindow_size,
		history_border_color,
		&[(spin_history_item_updater_fn, model_update_rate)],

		(0..num_spins_shown_in_history).map(|i| {
			let x_start = i as f64 * sub_width;
			let x = x_start + rand_generator.gen_range(0.0..jitter_x);
			let y = rand_generator.gen_range(0.0..jitter_y);

			(Vec2f::new(x, y), None)
		})
	));

	//////////

	spinitron_windows
}
