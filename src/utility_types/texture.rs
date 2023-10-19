use sdl2;
use sdl2::image::LoadTexture;

use crate::request;
use crate::window_tree::CanvasSDL;
use crate::utility_types::generic_result::GenericResult;

/* Note that the handle is wrapped in a struct, so that it can't be modified.

TODO: make a function called `update_texture_behind_handle`,
and don't allow some textures to use it.

Suppose that two copies of a texture handle exist within the program,
and then one function that has one of the handles changes the texture there.
That is then basically shared mutable access for a given texture, while
escaping the borrow checker. In cases where this should not be allowed,
consider giving some handles a flag that shared mutable access is not allowed.
For this though, I would have to make sure that if the function is used, that only
one copy of the handle exists. So, every time the handle is copied or cloned, then I would have
to keep some kind of internal reference count. TODO: perhaps I can use `RefCell` for that.

TODO: overall for this, perhaps I can just not allow copying of texture handles?
That might guarantee this mutability thing at compile-time. Textures can still be lost
if they are reassigned, but that can only happen once. */
pub struct TextureHandle {
	handle: u16
}

pub struct TexturePool<'a> {
	textures: Vec<Texture<'a>>,
	texture_creator: &'a TextureCreator
}

type Texture<'a> = sdl2::render::Texture<'a>;
type TextureCreator = sdl2::render::TextureCreator<sdl2::video::WindowContext>;
type TextureHandleResult = GenericResult<TextureHandle>;

impl<'a> TexturePool<'a> {
	pub fn new(texture_creator: &'a TextureCreator) -> Self {
		Self {textures: Vec::new(), texture_creator}
	}

	pub fn draw_texture_to_canvas(&self, handle: &TextureHandle,
		canvas: &mut CanvasSDL, dest_rect: sdl2::rect::Rect) -> GenericResult<()> {

		Ok(canvas.copy(&self.textures[handle.handle as usize], None, dest_rect)?)
	}

	fn allocate_texture_in_pool(&mut self, texture: Texture<'a>) -> TextureHandleResult {
		self.textures.push(texture);
		Ok(TextureHandle {handle: (self.textures.len() - 1) as u16})
	}

	pub fn make_texture_from_path(&mut self, path: &str) -> TextureHandleResult {
		self.allocate_texture_in_pool(self.texture_creator.load_texture(path)?)
	}

	pub fn make_texture_from_url(&mut self, url: &str) -> TextureHandleResult {
		let request_result = request::get(url)?;
		let texture = self.texture_creator.load_texture_bytes(request_result.as_bytes())?;
		self.allocate_texture_in_pool(texture)
	}

	// TODO: allow for texture deletion too
}
