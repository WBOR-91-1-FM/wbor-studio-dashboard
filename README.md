# WBOR's On-Air-Studio Dashboard

- An in-studio dashboard for [WBOR 91.1 FM](https://wbor.org/), Bowdoin College's student-run radio station.

- Aggregates API data from:
  - [Spinitron](https://spinitron.com/) for now-playing show and song information.
  - [Twilio](https://www.twilio.com/) for "text-the-DJ" messages.
  - [Tomorrow.io](https://www.tomorrow.io/) for realtime weather updates.
  - [AzuraCast](https://github.com/AzuraCast/AzuraCast/) for streaming server status updates.

- Runs on a monitor in the on-air studio.
- Want to contribute, or have any questions? [Get in touch here.](https://wbor.org/contact)

<https://github.com/user-attachments/assets/8e64e88a-d347-480f-9769-66d297655cc8>

---

## Dependencies

- `homebrew` (macOS):

```sh
brew install rust sdl2 sdl2_image sdl2_ttf sdl2_gfx
```

- `dnf` (Fedora):

```sh
sudo dnf install rust cargo SDL2-devel SDL2_image-devel SDL2_ttf-devel SDL2_gfx-devel
```

- `apt-get` (Raspbian, Debian, and many others):

```sh
# Doing the second line just because `apt-get` only has an outdated version of the toolchain:
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
sudo apt-get install libsdl2-dev libsdl2-image-dev libsdl2-ttf-dev libsdl2-gfx-dev
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

- To run the dashboard on its computer, use `run_dashboard.sh`. Note that this is only made to work in the WBOR studio. If the dashboard ever panics, the script will sleep for a bit, and then try launching it again (while writing all output to a log file `project.log`). Also, if a Discord channel webhook is set up in `api_keys.json`, any crashes will automatically send a message to the channel associated with that webhook.

- To communicate with the dashboard, use `communicate_with_dashboard.sh`:
  - To send a surprise (an image that appears for some amount of time, with a certain random chance of appearing during some time of the day), pass these arguments: `surprise assets/<surprise_with_given_path>`. The surprise path must be one previously defined in `src/dashboard_defs/themes/shared_utils.rs`.
  <br>

  - To update the results from the Spinitron API immediately (instead of waiting for the next API call every `N` seconds), pass this argument: `spinitron_refresh`.
  <br>

  - And for instant Twilio updates, pass this argument: `twilio_refresh`.

---

## Fault Tolerance

This dashboard is meant to keep running for a long, long time, no matter what happens.

1. **Power outages**: in the studio, the `run_dashboard.sh` script is a login item, ensuring that if the station loses power, upon startup/login the dashboard will launch itself again.
2. **Fatal bugs/panics**: using the `run_dashboard.sh` script, any panics are covered: the script ensures that the dashboard boots itself up again (after a small waiting period). These fatal errors can then be examined in the logs.
3. **Network errors at launch**: if the core state relating to the dashboard can't be initialized (e.g. state regarding an API like Spinitron or Twilio), the problem is often because the network isn't up. In `main.rs`, if this state can't be initialized, the dashboard sleeps for a bit, and then tries again shortly after.
4. **Other errors**: errors arising from window updaters are printed out, not resulting in a crash. And with the `ContinuallyUpdated` type, if some state during any async updating results in an error, the older state is automatically reverted back to (these errors are also displayed to the screen). This ensures that state corresponding to any API (e.g. text messages fetched from Twilio) is eventually consistent.

All of this means is that the dashboard is incredibly resilient. Disconnect the power, turn off the network, do anything at all: it'll find a way to boot itself up again.

---

## Tests

The dashboard doesn't technically have any unit tests, but it has something even better:

- When a texture is updated on the dashboard, transition animations may be invoked.
- These include opacity easing between the old and new textures, as well as interpolating the aspect ratio between the two textures.
- It's hard to ensure that these easers always return values between 0 and 1 (due to weird floating-point edge cases), so I use [kani](https://github.com/model-checking/kani), a bit-precise model checker, to ensure that these functions are correct for all possible inputs.
<br>
- In `dashboard_defs/easing_fns.rs`, I've written a series of proofs that Kani verifies, to statically verify that these easers are correct!

To install `kani`, run this:

```sh
cargo install --locked kani-verifier
cargo kani setup
```

Next, to run the proofs, run `cargo kani`. Most of the easer proofs finish pretty quickly, but a few take a very long time. Expect all of them to be done in around ten minutes.
