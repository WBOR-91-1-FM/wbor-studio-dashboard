use crate::{
    spinitron::state::SpinitronState,
    texture::{FontInfo, TextureCreationInfo},
    window_tree_defs::{twilio::TwilioState, clock::ClockHands}
};

pub struct SharedWindowState<'a> {
	pub clock_hands: ClockHands,
	pub spinitron_state: SpinitronState,
	pub twilio_state: TwilioState,

	pub font_info: FontInfo<'a>,

	// This is used whenever a texture can't be loaded
	pub fallback_texture_creation_info: TextureCreationInfo<'a>
}
