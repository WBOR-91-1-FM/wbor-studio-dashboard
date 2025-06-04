use std::{
	borrow::Cow,
	collections::BTreeMap
};

use crate::{
	utils::hash::hash_obj,

	window_tree::{
		Window,
		ColorSDL,
		WindowContents,
		WindowUpdaterParams,
		TypicalWindowParams
	},

	dashboard_defs::{
		easing_fns,
		updatable_text_pattern,
		shared_window_state::SharedWindowState
	}
};

//////////

#[derive(Hash)]
pub struct ErrorState {
	errors: BTreeMap<&'static str, String> // Error source -> error message
}

impl ErrorState {
	pub fn new() -> Self {
		Self {errors: BTreeMap::new()}
	}

	pub fn report(&mut self, source: &'static str, err: String) {
		log::error!("Error from '{source}': '{err}'");

		self.errors.insert(source, err);
	}

	pub fn unreport(&mut self, source: &'static str) {
		self.errors.remove(source);
	}

	// This should only ever be called if the number of sources is greater than zero.
	fn make_message(&self) -> String {
		let mut message = String::new();

		let num_sources = self.errors.len();
		assert!(num_sources > 0);

		let last_source_index = num_sources - 1;
		let plural_suffix = if num_sources == 1 {""} else {"s"};

		for (i, (source, error)) in self.errors.iter().enumerate() {
			let (is_first_source, is_last_source) = (i == 0, i == last_source_index);
			let (subsection_ending, maybe_and) = if is_last_source {(".", "and ")} else {("", "")};

			let subsection = if is_first_source {
				format!("Error{plural_suffix} encountered from '{source}' ({error}){subsection_ending}")
			}
			else {
				format!(", {maybe_and}'{source}' ({error}){subsection_ending}")
			};

			message.push_str(subsection.as_str());
		}

		message
	}
}

//////////

pub fn make_error_window(typical_params: TypicalWindowParams,
	background_contents: WindowContents, text_color: ColorSDL) -> Window {

	type ErrorWindowState = Option<u64>; // This is the current hash of the error state (if `None`, not initialized yet)

	impl updatable_text_pattern::UpdatableTextWindowMethods for ErrorWindowState {
		fn should_skip_update(params: &mut WindowUpdaterParams) -> bool {
			let inner_shared_state = params.shared_window_state.get_mut::<SharedWindowState>();

			let wrapped_individual_state = params.window.get_state_mut
				::<updatable_text_pattern::UpdatableTextWindowFields<ErrorWindowState>>();

			let no_errors_to_display = inner_shared_state.error_state.errors.is_empty();

			let skip_update = match &mut wrapped_individual_state.inner {
				Some(prev_hash) => {
					let curr_hash = hash_obj(&inner_shared_state.error_state);

					if curr_hash == *prev_hash {
						// Nothing changed, so keep things the same. Skipping update.
						true
					}
					else {
						// Update the hash, and only skip updating if there's no errors to display.
						*prev_hash = curr_hash;
						no_errors_to_display
					}
				}

				None => {
					let hash = hash_obj(&inner_shared_state.error_state);
					wrapped_individual_state.inner = Some(hash);
					no_errors_to_display // Skipping update if not displaying any errors the first time.
				}
			};

			// Skipping drawing if not displaying any errors.
			params.window.set_draw_skipping(no_errors_to_display);

			skip_update
		}

		fn compute_within_updater<'a>(inner_shared_state: &'a SharedWindowState) -> updatable_text_pattern::ComputedInTextUpdater<'a> {
			(Cow::Borrowed(inner_shared_state.font_info), " ")
		}

		fn extract_text(&self, inner_shared_state: &SharedWindowState) -> Cow<str> {
			Cow::Owned(inner_shared_state.error_state.make_message())
		}

		fn extract_texture_contents(window_contents: &mut WindowContents) -> &mut WindowContents {
			let WindowContents::Many(all_contents) = window_contents
			else {panic!("The error window contents was expected to be a list!")};
			&mut all_contents[1]
		}
	}

	let fields = updatable_text_pattern::UpdatableTextWindowFields {
		inner: None,
		text_color,
		scroll_easer: easing_fns::scroll::LEFT_LINEAR,
		update_rate: typical_params.view_refresh_update_rate,
		border_info: typical_params.border_info,
		scroll_speed_multiplier: 0.3
	};

	let mut window = updatable_text_pattern::make_window(
		fields, typical_params.top_left, typical_params.size,
		WindowContents::Many(vec![background_contents, WindowContents::Nothing])
	);

	window.set_draw_skipping(true);
	window

}
