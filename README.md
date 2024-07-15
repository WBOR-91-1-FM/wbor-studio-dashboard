# WBOR's On-Air-Studio Dashboard

- An in-studio dashboard for [WBOR 91.1 FM](https://wbor.org/), Bowdoin College's student-run radio station.
- Runs on a little CRT monitor in the on-air studio. New features added (almost) every week!
- Want to contribute? [Get in touch here.](https://wbor.org/contact)

## Dependencies

For a quick and easy macOS dependency install, assuming you have [homebrew](https://brew.sh/) installed, run:

```zsh
brew install rust sdl2 sdl2_image sdl2_ttf
```

### [Rust](https://www.rust-lang.org/)

- macOS ([homebrew](https://brew.sh/)): `brew install rust`
- Fedora: `sudo dnf install rust cargo`
- Debian: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh` (`apt-get` only has an outdated version of the toolchain)

### [SDL](https://www.libsdl.org/)

- macOS ([homebrew](https://brew.sh/)): `brew install sdl2 sdl2_image sdl2_ttf`
- Fedora: `sudo dnf install SDL2-devel SDL2_image-devel SDL2_ttf-devel`
- Debian: `sudo apt-get install libsdl2-dev libsdl2-image-dev libsdl2-ttf-dev`

## Build and Run

First, make sure you've cloned this repo to your machine via:

```zsh
git clone https://github.com/WBOR-91-1-FM/wbor-studio-dashboard.git && cd wbor-studio-dashboard
```

Next, assuming you've installed the required [dependencies](#dependencies), run:

```zsh
cargo run --release
```

Or, with logging enabled:

```
RUST_LOG=wbor-studio-dashboard cargo run --release
```

## Scripts

- To delete recent Twilio messages, use `delete_twilio_msgs.sh`. Change the number of messages to delete in the script, and it'll delete that number of messages one-by-one.
- To send surprises to the dashboard (images that appear for a certain amount of time, with certain chances of appearing at any second), use `trigger_surprise.sh`. Just pass it the name of a surprise that you've set up in `dashboard.rs`, and it'll send it over a local socket which the dashboard reads from.

## TODO

- Features:
  - DJ tips popping up now and then (like a video game loading screen)
  - Display streaming server online status (determined by whether it pings?)

- Technical:
  - Make a small window that shows the dashboard uptime (don't use `chrono::Duration`, since that will limit the uptime to some number of weeks)
  - Continuous deployment
  - When an error happens, make it print a message on screen that says that they should reach out to the tech director, `wbor@bowdoin.edu` (make a log of the error on disk too)
  - Crop all Spinitron photos 1:1 square
  - Maybe put the bounding box definition one layer out (with the parent)
  - Eventually, avoid all possibilities of panics (so all assertions and unwraps should be gone)
  - Maybe draw rounded rectangles with `sdl_gfx` later on
  - Render a text drop shadow
  - Set more rendering hints later on, if needed (beyond just the scale quality)
  - Figure out how to do pixel-size-independent-rendering (use `sdl_canvas.set_scale` for that?)
  - For logging, write the current spin to a file once it updates
  - Clear Twilio message history every 2 weeks - or maybe do it from within the dashboard - checking the date via modulus? Ideally through our server.
  - Reduce the number of API calls to Spinitron - down the road have our campus messenging server talk with the Pi?
  - Maybe make a custom OpenGL renderer (may be more performant). Tricky parts would be text rendering, and keping everything safe. Perhaps Vulkan instead? Or something more general?
  - Make some functions const
  - Use SDL3 bindings
  - Investigate the occasionally high CPU usage on the Pi (like 300%!)
  - Could multiple update rates per window be useful?
  - Format all debug types with `<varname>:?` when possible
  - Use the max durations of Spinitron spins to reduce the number of API calls
  - Make a small window that shows the dashboard uptime (`chrono::Duration` should work for a long, long time)

- Fun ideas:
  - Run the dashboard on a PVM/BVM (less burn-in), or an original iMac, eventually?
  - Maybe give a retro theme to everything
  - Some little Mario-type character running around the edges of the screen (like 'That Editor' by Bisqwit)
  - Different themes per each dashboard setup: wooden, garden, neon retro, frutiger aero, etc.
    - Fall: leaves + drifting clouds over the screen
    - Summer: shining run rays
    - Spring: occasional rain with sun
    - Winter: snow
  - Avoid screen burn-in somehow on non-dynamic parts of the screen (ideas below):
    - Shut off at night (or just for a few hours)
    - Screensavers
    - Layout swap (move screen elements around with a rapid or smooth animation) (do once every 15 minutes or so?)
    - Theme swap (instant or gradual) (based on things like weather, season, time of day, holiday, simple dark/light mode for day/night)
  - Separate from the dashboard - Artist querying experiment with Twilio (maybe)

## Troubleshooting