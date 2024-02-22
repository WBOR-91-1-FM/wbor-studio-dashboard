use chrono::DateTime;
use std::collections::HashMap;

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

// TODO: include caller ID, and an image, if sent?
#[derive(Clone)]
struct MessageInfo {
	display_text: String,
	from: String,
	body: String,
	time_sent_utc: DateTime<chrono::Utc>,
	just_updated: bool
}

#[derive(Clone)]
struct TwilioStateData {
	account_sid: String,
	auth_token: String,

	max_num_messages_in_history: usize,
	message_history_duration: chrono::Duration,

	no_message_available_message: String,
	failed_to_get_message_message: String,

	// Mapping messages' URIs to info about them
	current_messages: HashMap<String, MessageInfo>,

	/*
	This is a `Vec` of message URIs that holds the messages in a chronogically sorted order.

	TODO:
	- How to avoid repeating the string allocations with `current_messages`?
	- Can I keep it sorted as I go, somehow, instead of fully resorting each time?
	*/
	historically_sorted_messages_by_uri: Vec<String>,

	just_cleared_all_messages: bool
}

pub struct TwilioState {
	continually_updated: ContinuallyUpdated<TwilioStateData>
}

//////////

impl TwilioStateData {
	fn new(account_sid: &str, auth_token: &str,
		max_num_messages_in_history: usize,
		message_history_duration: chrono::Duration) -> Self {

		Self {
			account_sid: account_sid.to_string(),
			auth_token: auth_token.to_string(),

			max_num_messages_in_history,
			message_history_duration,

			// TODO: put the max history duration into this string
			no_message_available_message: "No recent text messages! ".to_string(),
			failed_to_get_message_message: "Failed to get text messages! ".to_string(),

			current_messages: HashMap::new(),
			historically_sorted_messages_by_uri: vec![],

			just_cleared_all_messages: false
		}
	}

	fn get_message_string_with_time(curr_time_utc: DateTime<chrono::Utc>,
		time_sent_utc: &DateTime<chrono::Utc>, from: &str, body: &str) -> String {

		let duration = curr_time_utc - time_sent_utc;

		//////////

		let unit_pairs = [
			("day", duration.num_days()),
			("hour", duration.num_hours()),
			("minute", duration.num_minutes()),
			("second", duration.num_seconds())
		];

		for (unit_name, unit_amount) in unit_pairs {
			if unit_amount > 0 {
				return format!("{} {}{} ago from {}: '{}'. ", unit_amount, unit_name,
					if unit_amount == 1 {""} else {"s"}, from, body)
			}
		}

		format!("Right now from {}: '{}'", from, body)
	}

	fn update_current_messages(&mut self) -> GenericResult<()> {
		////////// Making a request, and getting a response

		let curr_time_utc = chrono::Utc::now();
		let history_cutoff = (curr_time_utc - self.message_history_duration).to_rfc2822();
		let encoded_history_cutoff = urlencoding::encode(&history_cutoff);

		let request_url = format!(
			"https://api.twilio.com/2010-04-01/Accounts/{}/Messages.json?PageSize={}&DateSent>={}",
			self.account_sid, self.max_num_messages_in_history, encoded_history_cutoff
		);

		use base64::{engine::general_purpose, Engine as _};
		let request_auth_base64 = general_purpose::STANDARD.encode(format!("{}:{}", self.account_sid, self.auth_token));

		let response = request::get_with_maybe_header( // TODO: do the request URL building thing instead
			&request_url, // TODO: cache the request, and why is there a 11200 error in the response?
			Some(("Authorization", format!("Basic {}", request_auth_base64).as_str()))
		)?;

		////////// Creating a map of incoming messages

		let json: serde_json::Value = serde_json::from_str(response.as_str()?)?;

		// This will always be in the range of 0 <= num_messages <= self.num_messages_in_history
		let json_messages = json["messages"].as_array().unwrap();

		let incoming_message_map: HashMap<_, _> = HashMap::from_iter(
			json_messages.iter().map(|message| {
				let message_field = |name| message[name].as_str().unwrap();

				let (uri, unparsed_time_sent_utc, from, body) = (
					message_field("uri"), message_field("date_sent"),
					message_field("from"), message_field("body")
				);

				let time_sent_utc: DateTime<chrono::Utc> = DateTime::parse_from_rfc2822(unparsed_time_sent_utc).unwrap().into();

				(uri, (from, body, time_sent_utc))
			})
		);

		////////// Step 1: remove cached messages not present in the returned request

		let num_messages_before_clearing = self.current_messages.len();

		self.current_messages.retain(|current_message_url, _| {
			incoming_message_map.contains_key(current_message_url.as_str())
		});

		self.just_cleared_all_messages = num_messages_before_clearing != 0 && self.current_messages.len() == 0;

		////////// Step 2: add new messages not present in the cache

		for new_message_url in incoming_message_map.keys() {
			let maybe_current_message = self.current_messages.get_mut(*new_message_url);

			if let Some(current_message) = maybe_current_message {
				// println!("The current messages already contain this fetched message!");

				let possibly_new_string = Self::get_message_string_with_time(
					curr_time_utc, &current_message.time_sent_utc, &current_message.from, &current_message.body);

				// Remark is as updated if its string changed
				current_message.just_updated = possibly_new_string != current_message.display_text;
				if current_message.just_updated {current_message.display_text = possibly_new_string;}
			}
			else {
				// println!("Adding new message!");
				let (from, body, time_sent_utc) = incoming_message_map[new_message_url];

				self.current_messages.insert(new_message_url.to_string(),
					MessageInfo {
						display_text: Self::get_message_string_with_time(curr_time_utc, &time_sent_utc, from, body),
						from: from.to_string(),
						body: body.to_string(),
						time_sent_utc,
						just_updated: true
					}
				);
			}
		}

		////////// Step 3: sort the messages that I have into a separate `Vec`

		self.historically_sorted_messages_by_uri = self.current_messages.iter().map(|(uri, _)| uri.clone()).collect();
		self.historically_sorted_messages_by_uri.sort_by_key(|uri| self.current_messages[uri].time_sent_utc);

		//////////

		assert!(self.current_messages.len() <= self.max_num_messages_in_history);
		assert!(self.current_messages.len() == self.historically_sorted_messages_by_uri.len());

		// println!("I have {} messages.\n---", self.current_messages.len());

		Ok(())
	}
}

impl Updatable for TwilioStateData {
	fn update(&mut self) -> GenericResult<()> {
		self.update_current_messages()
	}
}

/* TODO: eventually, integrate `new` into `Updatable`, and
reduce the boilerplate for the `Updatable` stuff in general */
impl TwilioState {
	pub fn new(
		account_sid: &str, auth_token: &str,
		max_num_messages_in_history: usize,
		message_history_duration: chrono::Duration) -> Self {

		let data = TwilioStateData::new(
			account_sid, auth_token, max_num_messages_in_history,
			message_history_duration
		);

		Self {continually_updated: ContinuallyUpdated::new(&data)}
	}

	pub fn update(&mut self) -> GenericResult<()> {
		self.continually_updated.update()
	}
}


//////////

pub fn make_twilio_window(
	top_left: Vec2f, size: Vec2f, update_rate: UpdateRate,
	text_color: ColorSDL, bg_color: ColorSDL) -> Window {

	struct TwilioWindowState {
		text_color: ColorSDL
	}

	fn twilio_updater_fn((window, texture_pool, shared_state, area_drawn_to_screen): WindowUpdaterParams) -> GenericResult<()> {
		let window_state: &TwilioWindowState = window.get_state();
		let inner_shared_state: &SharedWindowState = shared_state.get_inner_value();
		let twilio_state_data = shared_state.get_inner_value::<SharedWindowState>().twilio_state.continually_updated.get_data();

		//////////

		let sorted_message_uris = &twilio_state_data.historically_sorted_messages_by_uri;

		let (twilio_message, just_updated) = if let Some(most_recent_uri) = sorted_message_uris.last() {
			let most_recent = &twilio_state_data.current_messages[most_recent_uri];
			(&most_recent.display_text, most_recent.just_updated)
		}
		else {
			(&twilio_state_data.no_message_available_message, twilio_state_data.just_cleared_all_messages)
		};

		// println!("{}", if just_updated {">>> Updating the curr message!"} else {"<<< Keeping it the same..."});

		//////////

		let texture_creation_info = TextureCreationInfo::Text((
			&inner_shared_state.font_info,

			TextDisplayInfo {
				text: twilio_message,
				color: window_state.text_color,

				scroll_fn: |secs_since_unix_epoch| {
					let total_cycle_time = 8.0;
					let scroll_time_percent = 0.25;

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
			{text_display_info.text = &twilio_state_data.failed_to_get_message_message;} else {panic!();};

		//////////

		window.update_texture_contents(just_updated,
			texture_pool, &texture_creation_info, &fallback_texture_creation_info
		)?;

		Ok(())
	}

	//////////

	let twilio_window = Window::new(
		Some((twilio_updater_fn, update_rate)),

		DynamicOptional::new(TwilioWindowState {
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
