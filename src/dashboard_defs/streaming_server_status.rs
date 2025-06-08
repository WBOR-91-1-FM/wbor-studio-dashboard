use crate::{
	dashboard_defs::shared_window_state::SharedWindowState,

	window_tree::{
		Window,
		WindowContents,
		WindowUpdaterParams
	},

	utils::{
		request,
		vec2f::Vec2f,
		time::Duration,
		generic_result::*,
		update_rate::UpdateRate,
		dynamic_optional::DynamicOptional,
		continually_updated::{ContinuallyUpdated, ContinuallyUpdatable}
	}
};

#[derive(Clone)]
struct ServerStatusChecker {
	url: String,
	num_retries: u8
}

impl ServerStatusChecker {
	fn new(url: &str, num_retries: u8) -> Self {
		Self {url: url.to_owned(), num_retries}
	}
}

impl ContinuallyUpdatable for ServerStatusChecker {
	type Param = ();

	async fn update(&mut self, _: &Self::Param) -> MaybeError {
		for i in 0..self.num_retries {

			#[derive(serde::Deserialize)]
			struct StreamingServerStatusResponse {} // We don't need any fields here

			let result: GenericResult<StreamingServerStatusResponse> = request::get_as!(&self.url);

			match result {
				Ok(_) => {
					return Ok(());
				}

				Err(err) => {
					if i == self.num_retries - 1 {
						return Err(err);
					}

					continue;
				}
			}
		}

		unreachable!()
	}
}

// Checking the updater once every X seconds, and polling the API once every Y seconds
fn server_status_updater_fn(params: WindowUpdaterParams) -> MaybeError {
	let inner_shared_state = params.shared_window_state.get_mut::<SharedWindowState>();
	let individual_window_state = params.window.get_state_mut::<ContinuallyUpdated<ServerStatusChecker>>();
	individual_window_state.update((), &mut inner_shared_state.error_state);
	Ok(())
}

pub fn make_streaming_server_status_window(url: &str, api_update_rate: Duration,
	view_refresh_update_rate: UpdateRate, num_retries: u8) -> Window {

	let pinger_updater = ContinuallyUpdated::new(
		ServerStatusChecker::new(url, num_retries),
		(),
		"the online streaming server",
		api_update_rate
	);

	Window::new(
		vec![(server_status_updater_fn, view_refresh_update_rate)],
		DynamicOptional::new(pinger_updater),
		WindowContents::Nothing,
		None,
		Vec2f::ZERO,
		Vec2f::ZERO,
		vec![]
	)
}
