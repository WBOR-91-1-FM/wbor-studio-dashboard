use std::borrow::Cow;

use crate::{
	texture::{
		FontInfo,
		DisplayText,
		TextDisplayInfo,
		TextureCreationInfo,
		TextTextureScrollFn
	},

	window_tree::{
		Window,
		ColorSDL,
		WindowContents,
		WindowUpdaterParams
	},

	utility_types::{
		vec2f::Vec2f,
		update_rate::UpdateRate,
		generic_result::MaybeError,
		dynamic_optional::DynamicOptional
	},

	dashboard_defs::shared_window_state::SharedWindowState
};

//////////

pub trait UpdatableTextWindowMethods {
	fn should_skip_update(updater_params: &mut WindowUpdaterParams) -> bool;
	fn compute_within_updater<'a>(inner_shared_state: &'a SharedWindowState) -> Cow<'a, FontInfo>; // TODO: support more datatypes for this
	fn extract_text(&self) -> Cow<str>;
	fn extract_texture_contents(window_contents: &mut WindowContents) -> &mut WindowContents;
}

#[derive(Clone)]
pub struct UpdatableTextWindowFields<IndividualState> {
	pub inner: IndividualState,
	pub text_color: ColorSDL,
	pub scroll_fn: TextTextureScrollFn,
	pub update_rate: UpdateRate,
	pub maybe_border_color: Option<ColorSDL>
}

//////////

// TODO: use this in more places
pub fn make_window<IndividualState: UpdatableTextWindowMethods + Clone + 'static>(
	fields: UpdatableTextWindowFields<IndividualState>, top_left: Vec2f, size: Vec2f,
	initial_contents: WindowContents) -> Window {

	fn updater_fn<IndividualState: UpdatableTextWindowMethods + 'static>(mut params: WindowUpdaterParams) -> MaybeError {
		if IndividualState::should_skip_update(&mut params) {
			return Ok(());
		}

		let wrapped_individual_state = params.window.get_state::<UpdatableTextWindowFields<IndividualState>>();
		let inner_shared_state = params.shared_window_state.get::<SharedWindowState>();
		let extracted_text = wrapped_individual_state.inner.extract_text();

		let texture_creation_info = TextureCreationInfo::Text((
			IndividualState::compute_within_updater(inner_shared_state),

			TextDisplayInfo {
				text: DisplayText::new(&extracted_text).with_padding("", " "),
				color: wrapped_individual_state.text_color,
				pixel_area: params.area_drawn_to_screen,
				scroll_fn: wrapped_individual_state.scroll_fn
			}
		));

		let texture_contents = IndividualState::extract_texture_contents(
			params.window.get_contents_mut()
		);

		texture_contents.update_as_texture(true, params.texture_pool,
			&texture_creation_info, &texture_creation_info)
	}

	//////////

	Window::new(
		Some((updater_fn::<IndividualState>, fields.update_rate)),
		DynamicOptional::new(fields.clone()),
		initial_contents,
		fields.maybe_border_color,
		top_left,
		size,
		None
	)
}
