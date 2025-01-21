use crate::{
    spinitron::state::SpinitronState,
    texture::pool::{FontInfo, TextureCreationInfo},
    dashboard_defs::{twilio::TwilioState, clock::ClockHands, error::ErrorState}
};

pub struct SharedWindowState<'a> {
	pub clock_hands: ClockHands,
	pub spinitron_state: SpinitronState,
	pub twilio_state: TwilioState,
	pub error_state: ErrorState,

	pub font_info: &'a FontInfo,

	// This is used whenever a texture can't be loaded
	pub get_fallback_texture_creation_info: fn() -> TextureCreationInfo<'a>,

	pub rand_generator: rand::rngs::ThreadRng

	/* TODO: can I keep the texture pool here, instead of passing it in to
	each window on its own (and the shared window state updater)? */
}
