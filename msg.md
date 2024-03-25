Lots of changes:

- main.rs: change a type.
- request.rs: lengthen the timeout time for requests from 5 to 10.
- spinitron/model.rs: do a bunch of tomfoolery to make it possible to make the spin image have a custom size (in this case, matched perfectly to the window size).
- texture.rs: make the path and URL Cow's in TextureCreationInfo, which allows for more flexible lifetime handling of strings in other parts of the code.
- update_rate.rs: add a FPS typedef. Make Seconds, FPS, and FrameIndex bigger, so that they can accomodate for higher framerates (overflow warnings happen otherwise).
- vec2f.rs: add a PartialEq derivation to Vec2f, for use in window_tree.rs.
- window_tree.rs: Add a PartialEq derivation to WindowContents, and add a get_contents function for Window, both for use in spinitron.rs.
- clock.rs: use the new TextureCreationInfo definition here.
- spinitron.rs: add an optimization where I don't recreate the model texture creation info when needed. Also incorporate the perfectly-sized-image logic with get_texture_creation_info.
- twilio.rs: fix (as much as possible) a bug with messages coming in the same second, and being in scrambled order. The solution is not the world's most solid one, but the code remains neat, and there's nothing bad about it.
- weather.rs: clean up an import block. Use a typedef newly defined in update_rate.rs.
- window_tree_defs.rs: use the new TextureCreationInfo definition here.
