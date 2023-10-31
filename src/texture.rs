use sdl2::{self, image::LoadTexture};

use crate::{
	request,
	window_tree::CanvasSDL,
	utility_types::generic_result::GenericResult
};

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

/* TODO: add options for possible color and alpha mods,
and a blend mode (those would go in a struct around this enum) */
pub enum TextureCreationInfo<'a> {
	Path(&'a str),
	Url(&'a str)
	// TODO: add an option for text later
}

pub struct TextureHandle {
	handle: u16
}

pub struct TexturePool<'a> {
	textures: Vec<Texture<'a>>,
	texture_creator: &'a TextureCreator
}

// TOOD: remove
impl std::fmt::Debug for TexturePool<'_> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
		write!(f, "[")?;

		for (index, texture) in self.textures.iter().enumerate() {
			write!(f, "({index}, {:?})", texture.query())?;
		}

		write!(f, "]")?;

		Ok(())
	}
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

	fn make_raw_texture(&mut self, creation_info: TextureCreationInfo) -> GenericResult<Texture<'a>> {
		let texture = match creation_info {
			TextureCreationInfo::Path(path) => {
				self.texture_creator.load_texture(path)
			},
			TextureCreationInfo::Url(url) => {
				/* Normally, the textures are 170x170 (this is described in the URL). If the scale factor isn't a box,
				just the smallest dimension will be picked. But, the size can be modified to anything desired.
				TODO: on the right URL format, resize the image to the given window box size by tweaking the URL
				(but do it from the Spinitron side of things). */

				let request_result = request::get(&url)?;
				self.texture_creator.load_texture_bytes(request_result.as_bytes())
			}
		};

		Ok(texture?)
	}

	pub fn make_texture(&mut self, creation_info: TextureCreationInfo) -> TextureHandleResult {
		let texture = self.make_raw_texture(creation_info)?;
		self.textures.push(texture);
		Ok(TextureHandle {handle: (self.textures.len() - 1) as u16})
	}

	pub fn remake_texture(&mut self, handle: &TextureHandle, creation_info: TextureCreationInfo) -> GenericResult<()> {
		self.textures[handle.handle as usize] = self.make_raw_texture(creation_info)?;
		Ok(())
	}

	// TODO: allow for texture deletion too
}
