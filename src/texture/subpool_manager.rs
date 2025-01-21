use std::collections::HashMap;

use crate::{
	utility_types::generic_result::*,

	texture::texture::{
		TextureHandle, TextureCreationInfo,
		RemakeTransitionInfo, TexturePool
	}
};

////////// This is used for managing a subset of textures used in the texture pool

// TODO: could I keep 2 piles instead (one for unused, and one for used)?
pub struct TextureSubpoolManager {
	subpool: HashMap<TextureHandle, bool>, // The boolean is true if it's used, otherwise unused
	max_size: usize // TODO: can I avoid keeping this here?
}

impl TextureSubpoolManager {
	pub fn new(subpool_size: usize) -> Self {
		Self {subpool: HashMap::with_capacity(subpool_size), max_size: subpool_size}
	}

	pub fn request_slot(&mut self, texture_creation_info: &TextureCreationInfo,
		maybe_remake_transition_info: Option<&RemakeTransitionInfo>,
		texture_pool: &mut TexturePool) -> GenericResult<TextureHandle> {

		assert!(self.subpool.len() <= self.max_size);

		// If this is the case, go and check for unused variants
		if self.subpool.len() == self.max_size {
			for (texture, is_used) in &mut self.subpool {
				if !*is_used {
					// println!("(request) doing re-request, and setting {texture:?} to used");
					*is_used = true;
					texture_pool.remake_texture(texture_creation_info, texture, maybe_remake_transition_info)?;
					return Ok(texture.clone());
				}
			}

			panic!("No textures available for requesting in subpool!");
		}
		else {
			let texture = texture_pool.make_texture(texture_creation_info)?;

			if self.subpool.insert(texture.clone(), true).is_some() {
				panic!("This texture was already allocated in the subpool!");
			}

			// println!("(request) setting {texture:?} to used");

			Ok(texture)
		}
	}

	pub fn re_request_slot(&mut self,
		incoming_texture: &TextureHandle,
		texture_creation_info: &TextureCreationInfo,
		maybe_remake_transition_info: Option<&RemakeTransitionInfo>,
		texture_pool: &mut TexturePool) -> MaybeError {

		if let Some(is_used) = self.subpool.get(incoming_texture) {
			// println!("(re-request) checking {incoming_texture:?} for being used before");
			assert!(is_used);
			// println!("(re-request) doing re-request for {incoming_texture:?}");
			texture_pool.remake_texture(texture_creation_info, incoming_texture, maybe_remake_transition_info)
		}
		else {
			panic!("Slot was not previously allocated in subpool!");
		}
	}

	// TODO: would making the incoming texture `mut` stop further usage of it?
	pub fn give_back_slot(&mut self, incoming_texture: &TextureHandle) {
		if let Some(is_used) = self.subpool.get_mut(incoming_texture) {
			// println!("(give back) checking {incoming_texture:?} for being used before");
			assert!(*is_used);
			// println!("(give back) setting {incoming_texture:?} to unused");
			*is_used = false;
		}
		else {
			panic!("Incoming texture did not already exist in subpool!");
		}
	}
}
