fn main() {
	// Use pkg-config to find and link against SDL2
	pkg_config::Config::new()
		.probe("sdl2")
		.unwrap();
}
