# WBOR's On-Air Studio Dashboard

- An in-studio dashboard for [WBOR 91.1 FM](https://wbor.org/), Bowdoin College's student-run radio station.
- Runs on a little CRT monitor in the on-air studio.
- Currently in development.

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

## Troubleshooting
