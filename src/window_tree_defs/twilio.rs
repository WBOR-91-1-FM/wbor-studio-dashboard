use crate::{
	utility_types::{
		vec2f::Vec2f,
		update_rate::UpdateRate,
		generic_result::GenericResult,
		dynamic_optional::DynamicOptional,
		thread_task::{Updatable, ContinuallyUpdated}
	},

	request,
	texture::{TextDisplayInfo, TextureCreationInfo},
	window_tree_defs::shared_window_state::SharedWindowState,
	window_tree::{ColorSDL, WindowContents, WindowUpdaterParams, Window}
};

//////////

/* TODO:
- Can I get caller ID from this?
- If a user sends an image, get that too
*/
#[derive(serde::Deserialize, Clone)]
struct MessageInfo {
	// There are more fields in the corresponding JSON that I am not using yet
	body: String,
	from: String,
	uri: String
}

#[derive(Clone)]
struct TwilioState {
	account_sid: String,
	auth_token: String,

	no_message_available_message: String,
	failed_to_get_message_message: String,

	current_message: Option<MessageInfo>,
	just_received_new_message: bool
}

impl TwilioState {
	fn new(account_sid: &str, auth_token: &str) -> Self {
		Self {
			account_sid: account_sid.to_string(),
			auth_token: auth_token.to_string(),

			no_message_available_message: "No text messages available! ".to_string(),
			failed_to_get_message_message: "Failed to get text messages! ".to_string(),

			current_message: None,
			just_received_new_message: false
		}
	}

	fn make_request(&self) -> GenericResult<minreq::Response> {
		let date = chrono::Utc::now().format("%Y-%m-%d");
		let num_messages_to_ask_for = 1; // This may change later, since I might display more messages at a time

		let request_url = format!( // TODO: do the request URL building thing instead
			"https://api.twilio.com/2010-04-01/Accounts/{}/Messages.json?DateSent={}&PageSize={}",
			self.account_sid, date, num_messages_to_ask_for
		);

		use base64::{engine::general_purpose, Engine as _};
		let request_auth_base64 = general_purpose::STANDARD.encode(format!("{}:{}", self.account_sid, self.auth_token));

		request::get_with_maybe_header(
			&request_url, // TODO: cache the request
			Some(("Authorization", format!("Basic {}", request_auth_base64).as_str()))
		)
	}
}

impl Updatable for TwilioState {
	fn update(&mut self) -> GenericResult<()> {
		let response = self.make_request()?;
		let json: serde_json::Value = serde_json::from_str(response.as_str()?)?;

		//////////

		let messages_json = &json["messages"];

		let num_messages = if let serde_json::Value::Array(inner_messages_json) = messages_json {
			Ok(inner_messages_json.len())
		}
		else {
			Err("Expected an array of messages from Twilio!".to_string())
		}?;

		//////////

		self.just_received_new_message = false;

		if num_messages == 0 {
			// TODO: will this result in a texture pool leak possibly once a day?
			self.current_message = None;
		}
		else {
			let newest = &messages_json[0];

			if let Some(prev_message) = &self.current_message {
				// Replacing the old message with the new one if needed
				if newest["uri"] != prev_message.uri {
					self.current_message = Some(serde_json::from_value(newest.clone())?);
					self.just_received_new_message = true;
				}
			}
			else {
				// Build an initial message
				self.current_message = Some(serde_json::from_value(newest.clone())?);
				// TODO: should this also set `self.just_received_new_message`?
			}
		}

		Ok(())
	}
}

//////////

pub fn make_twilio_window(
	top_left: Vec2f, size: Vec2f,
	update_rate: UpdateRate,
	text_color: ColorSDL,
	bg_color: ColorSDL,
	account_sid: &str,
	auth_token: &str) -> Window {

	struct TwilioWindowState {
		twilio_state: ContinuallyUpdated<TwilioState>,
		text_color: ColorSDL
	}

	fn twilio_updater_fn((window, texture_pool, shared_state, area_drawn_to_screen): WindowUpdaterParams) -> GenericResult<()> {
		// TODO: test the state transition between no msgs today -> a new msg (also for the boundary between days)
		{window.get_state_mut::<TwilioWindowState>().twilio_state.update()?;}

		let window_state: &TwilioWindowState = window.get_state();
		let inner_twilio_state = window_state.twilio_state.get_data();
		let inner_shared_state: &SharedWindowState = shared_state.get_inner_value();

		let twilio_message = if let Some(message) = &inner_twilio_state.current_message {
			format!("{}: \"{}\" ", message.from, message.body)
		} else {
			inner_twilio_state.no_message_available_message.clone()
		};

		let texture_creation_info = TextureCreationInfo::Text((
			&inner_shared_state.font_info,

			TextDisplayInfo {
				text: twilio_message,
				color: window_state.text_color,

				scroll_fn: |secs_since_unix_epoch| {
					let total_cycle_time = 8.0;
					let scroll_time_percent  = 0.25;

					let wait_boundary = total_cycle_time * scroll_time_percent;
					let scroll_value = secs_since_unix_epoch % total_cycle_time;

					let scroll_fract = if scroll_value < wait_boundary {scroll_value / wait_boundary} else {0.0};
					(scroll_fract, true)
				},

				max_pixel_width: area_drawn_to_screen.width(),
				pixel_height: area_drawn_to_screen.height()
			}
		));

		//////////

		let mut fallback_texture_creation_info = texture_creation_info.clone();

		if let TextureCreationInfo::Text((_, text_display_info)) = &mut fallback_texture_creation_info
			{text_display_info.text = inner_twilio_state.failed_to_get_message_message.clone();} else {panic!();};

		//////////

		window.update_texture_contents(inner_twilio_state.just_received_new_message,
			texture_pool, &texture_creation_info, &fallback_texture_creation_info
		)?;

		Ok(())
	}

	//////////

	let state = TwilioState::new(account_sid, auth_token);

	let twilio_window = Window::new(
		Some((twilio_updater_fn, update_rate)),

		DynamicOptional::new(TwilioWindowState {
			twilio_state: ContinuallyUpdated::new(&state),
			text_color
		}),

		WindowContents::Nothing,
		None,
		Vec2f::ZERO,
		Vec2f::ONE,
		None
	);

	Window::new(
		None,
		DynamicOptional::NONE,
		WindowContents::Color(bg_color),
		None,
		top_left,
		size,
		Some(vec![twilio_window])
	)
}
