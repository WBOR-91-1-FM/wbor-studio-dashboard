use std::borrow::Cow;

use crate::{
	texture::{
		pool::TextureCreationInfo,
		text::{FontInfo, DisplayText, TextDisplayInfo, TextTextureScrollEaser}
	},

	window_tree::{
		Window,
		ColorSDL,
		WindowContents,
		WindowBorderInfo,
		WindowUpdaterParams
	},

	utils::{
		vec2f::Vec2f,
		generic_result::*,
		update_rate::UpdateRate,
		dynamic_optional::DynamicOptional
	},

	dashboard_defs::shared_window_state::SharedWindowState
};

//////////

// Modified font info, and right display string padding
pub type ComputedInTextUpdater<'a> = (Cow<'a, FontInfo>, &'static str);

pub trait UpdatableTextWindowMethods {
	fn should_skip_update(params: &mut WindowUpdaterParams) -> bool;
	fn compute_within_updater<'a>(inner_shared_state: &'a SharedWindowState) -> ComputedInTextUpdater<'a>;
	fn extract_text(&self, inner_shared_state: &SharedWindowState) -> Cow<str>;
	fn extract_texture_contents(window_contents: &mut WindowContents) -> &mut WindowContents;
}

#[derive(Clone)]
pub struct UpdatableTextWindowFields<IndividualState> {
	pub inner: IndividualState,
	pub text_color: ColorSDL,
	pub scroll_easer: TextTextureScrollEaser,
	pub scroll_speed_multiplier: f64,
	pub update_rate: UpdateRate,
	pub border_info: WindowBorderInfo
}

//////////

// TODO: use this in more places
pub fn make_window<IndividualState: UpdatableTextWindowMethods + 'static>(
	fields: UpdatableTextWindowFields<IndividualState>, top_left: Vec2f, size: Vec2f,
	initial_contents: WindowContents) -> Window {

	fn updater_fn<IndividualState: UpdatableTextWindowMethods + 'static>(mut params: WindowUpdaterParams) -> MaybeError {
		if IndividualState::should_skip_update(&mut params) {
			return Ok(());
		}

		let wrapped_individual_state = params.window.get_state::<UpdatableTextWindowFields<IndividualState>>();
		let inner_shared_state = params.shared_window_state.get::<SharedWindowState>();
		let extracted_text = wrapped_individual_state.inner.extract_text(inner_shared_state);

		let (modified_font_info, right_padding) = IndividualState::compute_within_updater(inner_shared_state);

		let texture_creation_info = TextureCreationInfo::Text((
			modified_font_info,

			TextDisplayInfo::new(
				DisplayText::new(&extracted_text).with_padding("", right_padding),
				wrapped_individual_state.text_color,
				params.area_drawn_to_screen,
				wrapped_individual_state.scroll_easer,
				wrapped_individual_state.scroll_speed_multiplier
			)
		));

		let texture_contents = IndividualState::extract_texture_contents(
			params.window.get_contents_mut()
		);

		texture_contents.update_as_texture(
			true,
			params.texture_pool,
			&texture_creation_info,
			None,
			inner_shared_state.get_fallback_texture_creation_info
		)
	}

	//////////

	let border_info = fields.border_info;

	Window::new(
		vec![(updater_fn::<IndividualState>, fields.update_rate)],
		DynamicOptional::new(fields),
		initial_contents,
		border_info,
		top_left,
		size,
		vec![]
	)
}
