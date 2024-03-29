use crate::{
    spinitron::state::SpinitronState,
    texture::{FontInfo, TextureCreationInfo},
    dashboard_defs::{twilio::TwilioState, clock::ClockHands}
};

pub struct SharedWindowState<'a> {
	pub clock_hands: ClockHands,
	pub spinitron_state: SpinitronState,
	pub twilio_state: TwilioState<'a>,

	pub font_info: &'a FontInfo,

	// This is used whenever a texture can't be loaded
	pub fallback_texture_creation_info: TextureCreationInfo<'a>,

	pub dashboard_error: Option<String>

	/* TODO: can I keep the texture pool here, instead of passing it in to
	each window on its own (and the shared window state updater)? */
}
