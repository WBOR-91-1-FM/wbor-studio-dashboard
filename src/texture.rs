use sdl2;
use crate::request;
use crate::window_hierarchy::CanvasSDL;
use crate::generic_result::GenericResult;

#[derive(Copy, Clone)]
pub struct TextureHandle {
	handle: u16 // In a struct, so that it can't be modified
}

pub struct TexturePool<'a> {
	textures: Vec<Texture<'a>>,
	texture_creator: &'a TextureCreator
}

type Texture<'a> = sdl2::render::Texture<'a>;
type TextureCreator = sdl2::render::TextureCreator<sdl2::video::WindowContext>;
type TextureHandleResult = GenericResult<TextureHandle>;

impl<'a> TexturePool<'a> {
	pub fn new(texture_creator: &TextureCreator) -> TexturePool {
		TexturePool {textures: Vec::new(), texture_creator}
	}

	pub fn draw_texture_to_canvas(&self, texture: TextureHandle,
		canvas: &mut CanvasSDL, dest_rect: sdl2::rect::Rect) -> GenericResult<()> {

		Ok(canvas.copy(&self.textures[texture.handle as usize], None, dest_rect)?)
	}

	fn allocate_texture_in_pool(&mut self, texture: Texture<'a>) -> TextureHandleResult {
		self.textures.push(texture);
		Ok(TextureHandle {handle: (self.textures.len() - 1) as u16})
	}

	pub fn make_texture_from_path(&mut self, path: &str) -> TextureHandleResult {
		let surface = sdl2::surface::Surface::load_bmp(path)?;
		let texture = self.texture_creator.create_texture_from_surface(surface)?;
		self.allocate_texture_in_pool(texture)
	}

	pub fn make_texture_from_url(&mut self, url: &str) -> TextureHandleResult {
		use sdl2::image::LoadTexture;

		let request_result = request::get(url)?;
		let texture = self.texture_creator.load_texture_bytes(request_result.as_bytes())?;
		self.allocate_texture_in_pool(texture)
	}

	// TODO: allow for texture deletion too
}
