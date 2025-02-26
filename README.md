# WBOR's On-Air-Studio Dashboard

- An in-studio dashboard for [WBOR 91.1 FM](https://wbor.org/), Bowdoin College's student-run radio station.
- Runs on a little CRT monitor in the on-air studio.
- Want to contribute, or have any questions? [Get in touch here.](https://wbor.org/contact)

https://github.com/user-attachments/assets/8e64e88a-d347-480f-9769-66d297655cc8

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

- `apt-get` (Raspbian, Debian, and many others):

```sh
# Doing the second line just because `apt-get` only has an outdated version of the toolchain:
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

## Themes

The current themes are:
- `standard`
- `barebones`
- `retro_room`

You can change the theme in `app_config.json`.
If you want to make a new theme, start off by copying `standard.rs`, and make your changes from there. Make sure that it doesn't differ too much by running `diff standard.rs <YOUR_NEW_THEME.rs> --color=auto`.

---

## Scripts

- To delete recent Twilio messages, use `delete_twilio_msgs.sh`. Change the number of messages to delete in the script, and it'll delete that number of messages one-by-one.
- To send surprises to the dashboard (images that appear for a certain amount of time, with certain chances of appearing at any second), use `trigger_surprise.sh`. Just pass it the name of a surprise that you've set up in one of your theme files (e.g. `standard.rs`), and it'll send it over a local socket which the dashboard reads from.
- To run the dashboard on its computer, use `run_dashboard.sh`. Note that this is only made to work in the WBOR studio. If the dashboard ever panics, the script will sleep for a bit, and then try launching it again (while writing all output to a log file `project.log`).

---

## Fault Tolerance

This dashboard is meant to keep running for a long, long time, no matter what happens.

1. Power outages: In the studio, the `run_dashboard.sh` script is a login item, ensuring that if the station loses power, upon startup/login the dashboard will launch itself again.
2. Fatal bugs/panics: using the `run_dashboard.sh` script, any panics are covered: the script ensures that the dashboard boots itself up again (after a small waiting period). These fatal errors can then be examined in the logs.
3. Network errors at launch: If the core state relating to the dashboard can't be initialized (e.g. state regarding an API like Spinitron or Twilio), the problem is often because the network isn't up. In `main.rs`, if this state can't be initialized, the dashboard sleeps for a bit, and then tries again shortly after.
4. Other errors: errors arising from window updaters are printed out, not resulting in a crash. And with the `ContinuallyUpdated` type, if some state during any async updating results in an error, the older state is automatically reverted back to (these errors are also displayed to the screen). This ensures that state corresponding to any API (e.g. text messages fetched from Twilio) is eventually consistent.

All of this means is that the dashboard is incredibly resilient. Disconnect the power, turn off the network, do anything at all: it'll find a way to boot itself up again.

---
