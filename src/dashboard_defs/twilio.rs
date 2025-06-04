use std::{
	sync::Arc,
	borrow::Cow
};

use serde::{Deserializer, Deserialize};

use crate::{
	dashboard_defs::{
		easing_fns,
		error::ErrorState,
		shared_window_state::SharedWindowState
	},

	texture::{
		text::{DisplayText, FontInfo, TextDisplayInfo},
		pool::{RemakeTransitionInfo, TextureCreationInfo, TexturePool}
	},

	utils::{
		ipc::*,
		time::*,
		request,
		vec2f::Vec2f,
		generic_result::*,
		dynamic_optional::DynamicOptional,
		continually_updated::{ContinuallyUpdatable, ContinuallyUpdated, ContinuallyUpdatedState},

		api_history_list::{
			ApiHistoryList, ApiHistoryListTextureManager, ApiHistoryListImplementer,
			ApiHistoryListSubWindowInfo, make_api_history_list_window
		}
	},

	window_tree::{
		Window,
		ColorSDL,
		PixelAreaSDL,
		WindowContents,
		WindowUpdaterParams,
		TypicalWindowParams
	}
};

//////////

type MessageId = u128; // This maps onto Twilio's 32-character SIDs
type MessageAgeData = Option<(&'static str, &'static str, i64)>;

#[derive(Clone)]
struct MessageHistoryListImplementer {}

struct MessageHistoryListImplementerParam {
	curr_time: ReferenceTimestamp,
	curr_history_cutoff_time: ReferenceTimestamp,
	reveal_texter_identities: bool
}

impl MessageHistoryListImplementer {
	fn get_message_age_data(curr_time: ReferenceTimestamp, time_sent: ReferenceTimestamp) -> MessageAgeData {
		let duration = curr_time - time_sent;

		/* TODO:
		- Use a macro to stop this repetitive naming
		- Add support for months and years (is that possible?)
		- Also, could overflow happen here?
		- Map phone numbers to random colors (or, display number location?)
		- Later on, if we need to save on space, perhaps just show the timestamp
		*/

		let age_pairs = [
			("week", duration.num_weeks()),
			("day", duration.num_days()),
			("hour", duration.num_hours()),
			("min", duration.num_minutes()),
			("sec", duration.num_seconds())
		];

		for (age_name, age_amount) in age_pairs {
			if age_amount > 0 {
				let plural_suffix = if age_amount == 1 {""} else {"s"};
				return Some((age_name, plural_suffix, age_amount));
			}
		}

		None
	}

	fn format_phone_number(number: &str, before: &str, after_1: &str, after_2: &str) -> String {
		let (country_code, area_code, telephone_prefix, line_number) = (
			&number[0..2], &number[2..5], &number[5..8], &number[8..12]
		);

		format!("{before}{country_code} ({area_code}) {telephone_prefix}-{line_number}{after_1}{after_2}")
	}

	fn make_message_display_text(age_data: MessageAgeData, body: &str, num_attachments: u32, maybe_from: Option<&str>) -> String {
		if body.is_empty() && num_attachments != 0 {
			let is_more_than_one = num_attachments > 1;
			let maybe_plural_s = if is_more_than_one {"s"} else {""};
			return format!("Media attachment{maybe_plural_s} sent! Not renderable at this time though, unfortunately.");
		}

		let display_text = if let Some((unit_name, plural_suffix, unit_amount)) = age_data {
			format!("{unit_amount} {unit_name}{plural_suffix} ago: '{body}'")
		}
		else {
			format!("Right now: '{body}'")
		};

		//////////

		if let Some(from) = maybe_from {
			Self::format_phone_number(from, "From ", ", ", &display_text)
		}
		else {
			display_text
		}
	}
}

impl ApiHistoryListImplementer for MessageHistoryListImplementer {
	type Key = MessageId;
	type NonNative = IncomingMessageInfo;
	type Native = MessageInfo;

	type Param = MessageHistoryListImplementerParam;
	type ResolveTextureCreationInfoParam = (&'static FontInfo, ColorSDL, PixelAreaSDL);
	type IntermediateTextureCreationInfo = ();

	fn may_need_to_sort_api_results() -> bool {
		true
	}

	fn compare(a: &IncomingMessageInfo, b: &IncomingMessageInfo) -> std::cmp::Ordering {
		// Note: the smallest unit of time in `time_sent` is seconds.
		match a.time_sent.cmp(&b.time_sent) {
			std::cmp::Ordering::Equal => {
				/* If the messages were sent within the same second, ordering issues can occur.
				When that happens, resort to basing the ordering on the time that it was loaded by the app
				(which corresponds to the order provided by Twilio). This is not fully reliable either
				(since Twilio has no ordering guarantee), but it serves as a more reliable fallback in general,
				and using this ordering seems to work for me in practice. */
				b.time_loaded_by_app.cmp(&a.time_loaded_by_app)
			}

			other => other
		}
	}

	fn get_key(offshore: &IncomingMessageInfo) -> MessageId {
		offshore.sid
	}

	fn is_expired(param: &Self::Param, offshore: &IncomingMessageInfo) -> bool {
		offshore.time_sent < param.curr_history_cutoff_time
	}

	fn create_new_local(param: &Self::Param, offshore: &IncomingMessageInfo) -> MessageInfo {
		let age_data = Self::get_message_age_data(param.curr_time, offshore.time_sent);
		let maybe_from = param.reveal_texter_identities.then(|| offshore.from.clone());
		let trimmed_body = offshore.body.trim().to_string();
		let num_attachments = offshore.num_attachments;

		let display_text = Self::make_message_display_text(
			age_data, &trimmed_body, num_attachments, maybe_from.as_deref()
		);

		MessageInfo {
			age_data,
			time_sent: offshore.time_sent,

			maybe_from,
			body: trimmed_body,
			display_text,

			num_attachments
		}
	}

	fn update_local(param: &Self::Param, local: &mut MessageInfo) -> bool {
		// Only making a new string if the age data became expired
		let age_data = Self::get_message_age_data(param.curr_time, local.time_sent);

		let just_updated = age_data != local.age_data;

		if just_updated {
			local.display_text = Self::make_message_display_text(
				age_data, &local.body, local.num_attachments, local.maybe_from.as_deref()
			);

			local.age_data = age_data;
		}

		just_updated
	}

	async fn get_intermediate_texture_creation_info(_: &Self::Param, _: &MessageInfo) -> Self::IntermediateTextureCreationInfo {}

	fn resolve_texture_creation_info<'a>(param: &Self::ResolveTextureCreationInfoParam, local: &MessageInfo, _: &'a ()) -> TextureCreationInfo<'a> {
		TextureCreationInfo::Text((
			Cow::Borrowed(param.0),

			TextDisplayInfo::new(
				// Ensuring that the displayed text is well-formed for on-screen rendering
				DisplayText::new(&local.display_text).with_padding("", " "),

				param.1,
				param.2,
				easing_fns::scroll::PAUSE_THEN_SCROLL_LEFT,
				1.0
			)
		))
	}
}

//////////

fn serde_parse_sid_to_u128<'de, D>(deserializer: D) -> Result<u128, D::Error> where D: Deserializer<'de> {
	let as_string = String::deserialize(deserializer)?;

	if as_string.len() != 34 {
		Err(serde::de::Error::custom("Twilio SIDs should be 34 chars!"))
	}
	else {
		u128::from_str_radix(&as_string[2..], 16).map_err(serde::de::Error::custom)
	}
}

fn serde_parse_string_to_u32<'de, D>(deserializer: D) -> Result<u32, D::Error> where D: Deserializer<'de> {
	let as_string = String::deserialize(deserializer)?;
	as_string.parse::<u32>().map_err(serde::de::Error::custom)
}

#[derive(Deserialize)]
struct IncomingMessageInfo {
	#[serde(rename = "sid", deserialize_with = "serde_parse_sid_to_u128")]
	sid: u128, // This is normally a string, but making it an integer here for easier comparisons

	// This field is originally called `date_created`, but calling it `time_sent` instead, which makes more sense
	#[serde(rename = "date_created", deserialize_with = "serde_parse::rfc2822_timestamp")]
	time_sent: ReferenceTimestamp,

	/* This field is not in the incoming message JSON, but is added by the app. It's used as a fallback key for sorting
	the incoming messages, since the time sent may not be granular enough (this has sub-second precision, but the time sent
	only goes down to 1 second). TODO: will the incoming message info structs be deserialized in order? If not, this would break that... */
	#[serde(skip_deserializing, default = "ReferenceTimezone::now")]
	time_loaded_by_app: ReferenceTimestamp,

	from: String,
	body: String,

	#[serde(rename = "num_media", deserialize_with = "serde_parse_string_to_u32")]
	num_attachments: u32
}

#[derive(Clone)]
struct MessageInfo {
	age_data: MessageAgeData,
	time_sent: ReferenceTimestamp,

	maybe_from: Option<String>, // This is `None` if the message identity is hidden
	body: String,
	display_text: String,

	num_attachments: u32
}

struct ImmutableTwilioStateData {
	account_sid: String,
	request_auth: String,
	max_num_messages_in_history_as_string: String,

	max_num_messages_in_history: usize,
	message_history_duration: Duration,
	reveal_texter_identities: bool,

	text_color: ColorSDL
}

#[derive(Clone)]
struct TwilioStateData {
	// Immutable fields (in an `Arc` so they are not needlessly copied during the continual updating):
	immutable: Arc<ImmutableTwilioStateData>,

	// Mutable fields:
	unformatted_and_formatted_phone_number: Option<(String, String)>,
	message_history_list: ApiHistoryList<MessageHistoryListImplementer>
}

pub struct TwilioState {
	just_got_new_continual_data: bool, // This is for when a new Twilio update has arrived
	continually_updated: ContinuallyUpdated<TwilioStateData>,
	instant_update_socket_listener: IpcSocketListener,
	message_history_list_texture_manager: ApiHistoryListTextureManager<MessageHistoryListImplementer>
}

//////////

impl TwilioStateData {
	fn new(account_sid: &str, auth_token: &str,
		max_num_messages_in_history: usize,
		message_history_duration: Duration,
		reveal_texter_identities: bool,
		text_color: ColorSDL) -> Self {

		use base64::{engine::general_purpose::STANDARD, Engine};
		let request_auth_base64 = STANDARD.encode(format!("{account_sid}:{auth_token}"));

		Self {
			immutable: Arc::new(ImmutableTwilioStateData {
				account_sid: account_sid.to_owned(),
				request_auth: "Basic ".to_owned() + &request_auth_base64,
				max_num_messages_in_history_as_string: max_num_messages_in_history.to_string(),

				max_num_messages_in_history,
				message_history_duration,

				reveal_texter_identities,
				text_color
			}),

			unformatted_and_formatted_phone_number: None,
			message_history_list: ApiHistoryList::new(max_num_messages_in_history)
		}
	}

	async fn do_twilio_request<T: for<'de> serde::Deserialize<'de>>
		(&self, endpoint: &str, path_params: &[Cow<'_, str>], query_params: &[(&str, Cow<'_, str>)]) -> GenericResult<T> {

		let base_url = format!("https://api.twilio.com/2010-04-01/Accounts/{}/{endpoint}.json", self.immutable.account_sid);
		let request_url = request::build_url(&base_url, path_params, query_params);

		// TODO: cache the constructed requests, and why is there a 11200 error in the response for messages?
		request::get_as!(&request_url, ("Authorization", &self.immutable.request_auth))
	}
}

impl ContinuallyUpdatable for TwilioStateData {
	type Param = ();

	async fn update(&mut self, _: &Self::Param) -> MaybeError {
		////////// Initializing the phone number if needed

		if self.unformatted_and_formatted_phone_number.is_none() {

			let response: serde_json::Value = self.do_twilio_request("IncomingPhoneNumbers", &[], &[]).await?;

			let numbers_json = response["incoming_phone_numbers"].as_array().unwrap();
			assert!(numbers_json.len() == 1);

			let number = &numbers_json[0]["phone_number"].as_str().unwrap();

			self.unformatted_and_formatted_phone_number = Some((
				number.to_string(),
				MessageHistoryListImplementer::format_phone_number(number, "Messages to ", ":", "")
			));
		}

		////////// Preparing to make a request

		let curr_time = ReferenceTimezone::now();
		let curr_history_cutoff_time = curr_time - self.immutable.message_history_duration;
		let curr_history_cutoff_day = curr_history_cutoff_time.format("%Y-%m-%d").to_string();

		let phone_number = self.unformatted_and_formatted_phone_number
			.as_ref().map(|(unformatted, _)| unformatted.as_str()).unwrap();

		////////// Making a request

		#[derive(serde::Deserialize)]
		struct TwilioMessageResponse {
			messages: Vec<IncomingMessageInfo>
		}

		/* This will always be in the range of 0 <= num_messages <= self.num_messages_in_history.
		TODO: Should I really limit the page size here? Twilio not returning messages in order might make this a problem... */
		let mut message_response: TwilioMessageResponse = self.do_twilio_request("Messages", &[],
			&[
				("To", Cow::Borrowed(phone_number)), // Adding this filters out all outbound messages
				("PageSize", Cow::Borrowed(&self.immutable.max_num_messages_in_history_as_string)),
				("DateSent%3E", Cow::Borrowed(&curr_history_cutoff_day)) // Note: the '%3E' is a URL-encoded '>'
			]
		).await?;

		////////// Updating the message history list from the response

		let implementer_param = MessageHistoryListImplementerParam {
			curr_time, curr_history_cutoff_time, reveal_texter_identities: self.immutable.reveal_texter_identities
		};

		self.message_history_list.update(&mut message_response.messages, &implementer_param).await;

		Ok(())
	}
}

impl TwilioState {
	pub async fn new(
		api_update_rate: Duration,
		account_sid_and_auth_token: (&str, &str),
		max_num_messages_in_history: usize,
		message_history_duration: Duration,
		reveal_texter_identities: bool,

		text_color: ColorSDL,
		maybe_remake_transition_info: Option<RemakeTransitionInfo>) -> GenericResult<Self> {

		let data = TwilioStateData::new(
			account_sid_and_auth_token.0, account_sid_and_auth_token.1,
			max_num_messages_in_history, message_history_duration, reveal_texter_identities, text_color
		);

		Ok(Self {
			just_got_new_continual_data: false,
			continually_updated: ContinuallyUpdated::new(data, (), "Twilio", api_update_rate),
			instant_update_socket_listener: make_ipc_socket_listener("twilio_instant_update").await?,
			message_history_list_texture_manager: ApiHistoryListTextureManager::new(max_num_messages_in_history, maybe_remake_transition_info)
		})
	}

	pub fn update(&mut self, error_state: &mut ErrorState,
		texture_pool: &mut TexturePool, font_info: &'static FontInfo, pixel_area: PixelAreaSDL) {

		////////// Check for an instant wakeup

		let do_ipc_socket_wakeup = try_listening_to_ipc_socket(&mut self.instant_update_socket_listener).is_some();

		if do_ipc_socket_wakeup {
			// The result of this wakeup may take until the next update iteration to be processed
			self.continually_updated.wake_up_if_sleeping();
		}

		////////// Check if we got new continual data or not

		// The pixel area and text color are passed to the message history list implementer
		let continual_state = self.continually_updated.update((), error_state);
		self.just_got_new_continual_data = continual_state == ContinuallyUpdatedState::GotNewData;
		if !self.just_got_new_continual_data {return;}

		//////////

		let twilio_state = self.continually_updated.get_curr_data();
		let param = (font_info, twilio_state.immutable.text_color, pixel_area);
		self.message_history_list_texture_manager.update_from_history_list(&twilio_state.message_history_list, texture_pool, &param)
	}
}

//////////

fn history_updater_fn(params: WindowUpdaterParams) -> MaybeError {
	let message_index = *params.window.get_state::<usize>();
	let inner_shared_state = params.shared_window_state.get_mut::<SharedWindowState>();
	let twilio_state = &mut inner_shared_state.twilio_state;

	//////////

	// The first window handles the updating
	if message_index == 0 {
		twilio_state.update(
			&mut inner_shared_state.error_state,
			params.texture_pool,
			inner_shared_state.font_info,
			params.area_drawn_to_screen
		);
	}

	if !twilio_state.just_got_new_continual_data {
		return Ok(());
	}

	//////////

	let maybe_texture = twilio_state.message_history_list_texture_manager.get_texture_at_index(
		message_index, &twilio_state.continually_updated.get_curr_data().message_history_list
	);

	*params.window.get_contents_mut() = if let Some(texture) = maybe_texture {
		WindowContents::Texture(texture)
	}
	else {
		WindowContents::Nothing
	};

	//////////

	Ok(())
}

fn top_box_updater_fn(params: WindowUpdaterParams) -> MaybeError {
	let inner_shared_state = params.shared_window_state.get::<SharedWindowState>();
	let twilio_state = inner_shared_state.twilio_state.continually_updated.get_curr_data();

	let WindowContents::Many(many) = params.window.get_contents_mut()
	else {panic!("The top box for Twilio did not contain a vec of contents!");};

	if let WindowContents::Nothing = many[1] {
		let formatted_number = match &twilio_state.unformatted_and_formatted_phone_number {
			Some((_, formatted_number)) => formatted_number,

			None => {
				return Ok(()); // Will check for the number again next time
			}
		};

		let texture_creation_info = TextureCreationInfo::Text((
			Cow::Borrowed(inner_shared_state.font_info),

			TextDisplayInfo::new(
				DisplayText::new(formatted_number).with_padding(" ", ""),
				twilio_state.immutable.text_color,
				params.area_drawn_to_screen,
				easing_fns::scroll::STAY_PUT,
				1.0
			)
		));

		many[1] = WindowContents::make_texture_contents(&texture_creation_info, params.texture_pool)?;
	}

	Ok(())
}

//////////

pub fn make_twilio_windows(
	typical_params: TypicalWindowParams,

	twilio_state: &TwilioState,
	top_box_height: f64,
	top_box_contents: WindowContents,
	message_text_zoom_factor: Vec2f,
	message_background_contents: WindowContents) -> Vec<Window> {

	let max_num_messages_in_history = twilio_state.continually_updated.get_curr_data().immutable.max_num_messages_in_history;
	let subwindow_size = Vec2f::new(1.0, 1.0 / max_num_messages_in_history as f64);

	let subwindow_info = (0..max_num_messages_in_history).map(|i|
		ApiHistoryListSubWindowInfo {
			top_left: Vec2f::new(0.0, i as f64 * subwindow_size.y()),
			main_window_zoom_factor: message_text_zoom_factor,
			background_contents: message_background_contents.clone(),
			skip_aspect_ratio_correction_for_background_contents: true
	});

	let message_history_window = make_api_history_list_window(
		(typical_params.top_left, typical_params.size),
		typical_params.border_info,
		subwindow_size,
		&[(history_updater_fn, typical_params.view_refresh_update_rate)],
		subwindow_info
	);

	let top_box_window = Window::new(
		vec![(top_box_updater_fn, typical_params.view_refresh_update_rate)],
		DynamicOptional::NONE,
		WindowContents::Many(vec![top_box_contents, WindowContents::Nothing]),
		None,
		Vec2f::new(typical_params.top_left.x(), typical_params.top_left.y() - top_box_height),
		Vec2f::new(typical_params.size.x(), top_box_height),
		vec![]
	);

	vec![message_history_window, top_box_window]
}
