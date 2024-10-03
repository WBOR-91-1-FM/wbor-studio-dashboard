# WBOR's On-Air-Studio Dashboard

- An in-studio dashboard for [WBOR 91.1 FM](https://wbor.org/), Bowdoin College's student-run radio station.
- Runs on a little CRT monitor in the on-air studio.
- Want to contribute, or have any questions? [Get in touch here.](https://wbor.org/contact)

---

## Dependencies

- `homebrew` (macOS):

```sh
brew install rust sdl2 sdl2_image sdl2_ttf
```

- `dnf` (Fedora):

```sh
sudo dnf install rust cargo SDL2-devel SDL2_image-devel SDL2_ttf-devel
```

- `apt-get` (Raspbian):

```sh
# Doing this just because `apt-get` only has an outdated version of the toolchain:
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

sudo apt-get install libsdl2-dev libsdl2-image-dev libsdl2-ttf-dev
```

---

## Build and Run

Assuming you've cloned the project and installed the required [dependencies](#dependencies), run the following from the project's root folder:

```sh
cargo run --release
```

Or, with logging enabled:

```sh
RUST_LOG=wbor_studio_dashboard cargo run --release
```

---

## Scripts

- To delete recent Twilio messages, use `delete_twilio_msgs.sh`. Change the number of messages to delete in the script, and it'll delete that number of messages one-by-one.
- To send surprises to the dashboard (images that appear for a certain amount of time, with certain chances of appearing at any second), use `trigger_surprise.sh`. Just pass it the name of a surprise that you've set up in `dashboard.rs`, and it'll send it over a local socket which the dashboard reads from.

---

## Branch Structure

- Each branch represents a theme. I am aiming for as little branch divergence as possible.
- A way help achieve this is by checking that only `dashboard.rs` (and maybe something in `assets/`) is modified for every theme.
- For any logic-based change to the dashboard's internals, make your change to the `main` branch (or via some other feature branch, and then merge to `main`), and after that, merge that change into every other theme branch.
- Changing the logic behind the dashboard in a theme branch is not encouraged (it should ideally be shared by every branch).
- This might not be the world's best system for maintaining different themes, but since so much of the code per theme in `dashboard.rs` is shared, it makes sense in this case (I think).

---

## [TODO](https://github.com/orgs/WBOR-91-1-FM/projects/3/views/1)
