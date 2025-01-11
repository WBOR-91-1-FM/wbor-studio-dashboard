use crate::{
	request,
	dashboard_defs::shared_window_state::SharedWindowState,

	window_tree::{
		Window,
		WindowContents,
		WindowUpdaterParams
	},

	utility_types::{
		vec2f::Vec2f,
		generic_result::*,
		update_rate::UpdateRate,
		dynamic_optional::DynamicOptional,
		continually_updated::{ContinuallyUpdated, Updatable}
	}
};

#[derive(Clone)]
struct ServerStatusChecker {
	url: &'static str,
	num_retries: u8
}

impl ServerStatusChecker {
	fn new(url: &'static str, num_retries: u8) -> Self {
		Self {url, num_retries}
	}
}

impl Updatable for ServerStatusChecker {
	type Param = ();

	async fn update(&mut self, _: &Self::Param) -> MaybeError {
		for _ in 0..self.num_retries {
			match request::get(self.url).await {
				Ok(_) => return Ok(()),
				Err(_) => continue
			}
		}

		error_msg!("Could not reach the streaming server!")
	}
}

fn server_status_updater_fn(params: WindowUpdaterParams) -> MaybeError {
	let inner_shared_state = params.shared_window_state.get_mut::<SharedWindowState>();
	let individual_window_state = params.window.get_state_mut::<ContinuallyUpdated<ServerStatusChecker>>();
	individual_window_state.update(&(), &mut inner_shared_state.error_state)?;
	Ok(())
}

pub async fn make_streaming_server_status_window(url: &'static str, ping_rate: UpdateRate, num_retries: u8) -> Window {
	let pinger_updater = ContinuallyUpdated::new(
		&ServerStatusChecker::new(url, num_retries),
		&(),
		"the online streaming server"
	).await;

	Window::new(
		Some((server_status_updater_fn, ping_rate)),
		DynamicOptional::new(pinger_updater),
		WindowContents::Nothing,
		None,
		Vec2f::ZERO,
		Vec2f::ZERO,
		None
	)
}
