# WBOR's On-Air-Studio Dashboard

- An in-studio dashboard for [WBOR 91.1 FM](https://wbor.org/), Bowdoin College's student-run radio station.
- Runs on a little CRT monitor in the on-air studio.
- Currently in early development.
- Want to contribute? [Get in touch here.](https://wbor.org/contact)

## Dependencies

### [Rust](https://www.rust-lang.org/)

- Homebrew: `brew install rust`
- Fedora: `sudo dnf install rust cargo`
- Debian: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh` (`apt-get` only has an outdated version of the toolchain)

### [SDL](https://www.libsdl.org/)

- Homebrew: `brew install sdl2 sdl2_image sdl2_ttf`
- Fedora: `sudo dnf install SDL2-devel SDL2_image-devel SDL2_ttf-devel`
- Debian: `sudo apt-get install libsdl2-dev libsdl2-image-dev libsdl2-ttf-dev`

## Build and Run

- `cargo run --release`

Or with logging:

- `RUST_LOG=wbor-studio-dashboard cargo run --release`

## TODO

- Features:
  - DJ tips popping up now and then (like a video game loading screen)
  - Display streaming server online status (determined by whether it pings?) address is: 161.35.248.7
  - User interaction with the dashboard via the [Stream Deck](https://timothycrosley.github.io/streamdeck-ui/) (toggle display elements, scroll through texts, block a text sender, etc.)
  - Finish the background image (vary it based on the theme?)
  - Add a 'no recent spins' message if there are no spins in the last 60 minutes
  - Show the album history on the bookshelf

- Technical:
  - CI/CD
  - When an error happens, make it print a message on screen that says that they should reach out to the tech director, `wbor@bowdoin.edu` (make a log of the error on disk too)
  - Crop all Spinitron photos 1:1 square
  - Maybe put the bounding box definition one layer out (with the parent)
  - Abstract the main loop out, so that just some data and fns are passed into it
  - Eventually, avoid all possibilities of panics (so all assertions and unwraps should be gone)
  - Maybe draw rounded rectangles with `sdl_gfx` later on
  - Render a text drop shadow
  - Set more rendering hints later on, if needed (beyond just the scale quality)
  - Figure out how to do pixel-size-independent-rendering (use `sdl_canvas.set_scale` for that?)
  - For logging, write the current spin to a file once it updates
  - Make a little script on the Pi to clear the message history every 2 weeks - or maybe do it from within the dashboard - checking the date via modulus?
  - Use the max durations of Spinitron spins to reduce the number of API calls
  - Maybe make a custom OpenGL renderer (may be more performant). Tricky parts would be text rendering, and keping everything safe. Perhaps Vulkan instead? Or something more general?
  - Make some functions const
  - Use SDL3 bindings
  - Make a small window that shows the dashboard uptime (don't use `chrono::Duration`, since that will limit the uptime to some number of weeks)

- Fun ideas:
  - Run the dashboard on a PVM, or an original iMac, eventually?
  - Maybe give a retro theme to everything
  - Some little Mario-type character running around the edges of the screen (like 'That Editor' by Bisqwit)
  - Different themes per each dashboard setup: wooden, garden, neon retro, frutiger aero, etc.
    - Fall: leaves + drifting clouds over the screen
    - Summer: shining run rays
    - Spring: occasional rain with sun
    - Winter: snow
  - Make Nathan Fielder pop up sometimes (at a random time, for a random amount of time, saying something random, e.g. "Hey. I'm proud of you.")
  - Avoid screen burn-in somehow on non-dynamic parts of the screen (ideas below):
    - Shut off at night (or just for a few hours)
    - Screensavers
    - Layout swap (move screen elements around with a rapid or smooth animation) (do once every 15 minutes or so?)
    - Theme swap (instant or gradual) (based on things like weather, season, time of day, holiday, simple dark/light mode for day/night)
    - Use a PVM/BVM (they have less burn-in)

## Troubleshooting
