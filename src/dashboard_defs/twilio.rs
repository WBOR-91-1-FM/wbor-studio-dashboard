use std::{
	sync::Arc,
	borrow::Cow,
	collections::HashMap
};

use crate::{
	request,

	utility_types::{
		time::*,
		vec2f::Vec2f,
		generic_result::*,
		update_rate::UpdateRate,
		dynamic_optional::DynamicOptional,
		continually_updated::{ContinuallyUpdated, Updatable}
	},

	dashboard_defs::{
		easing_fns,
		error::ErrorState,
		shared_window_state::SharedWindowState,
	},

	window_tree::{ColorSDL, Window, WindowContents, WindowUpdaterParams},
	texture::texture::{FontInfo, DisplayText, TextDisplayInfo, TextureCreationInfo, TextureHandle, TexturePool, RemakeTransitionInfo}
};

// TODO: split this file up into some smaller files

////////// This is used for managing a subset of textures used in the texture pool

// TODO: could I keep 2 piles instead (one for unused, and one for used)?
struct TextureSubpoolManager {
	subpool: HashMap<TextureHandle, bool>, // The boolean is true if it's used, otherwise unused
	max_size: usize // TODO: can I avoid keeping this here?
}

impl TextureSubpoolManager {
	fn new(subpool_size: usize) -> Self {
		Self {subpool: HashMap::with_capacity(subpool_size), max_size: subpool_size}
	}

	fn request_slot(&mut self, texture_creation_info: &TextureCreationInfo,
		maybe_remake_transition_info: Option<&RemakeTransitionInfo>,
		texture_pool: &mut TexturePool) -> GenericResult<TextureHandle> {

		assert!(self.subpool.len() <= self.max_size);

		// If this is the case, go and check for unused variants
		if self.subpool.len() == self.max_size {
			for (texture, is_used) in &mut self.subpool {
				if !*is_used {
					// println!("(request) doing re-request, and setting {texture:?} to used");
					*is_used = true;
					texture_pool.remake_texture(texture_creation_info, texture, maybe_remake_transition_info)?;
					return Ok(texture.clone());
				}
			}

			panic!("No textures available for requesting in subpool!");
		}
		else {
			let texture = texture_pool.make_texture(texture_creation_info)?;

			if self.subpool.insert(texture.clone(), true).is_some() {
				panic!("This texture was already allocated in the subpool!");
			}

			// println!("(request) setting {texture:?} to used");

			Ok(texture)
		}
	}

	fn re_request_slot(&mut self,
		incoming_texture: &TextureHandle,
		texture_creation_info: &TextureCreationInfo,
		maybe_remake_transition_info: Option<&RemakeTransitionInfo>,
		texture_pool: &mut TexturePool) -> MaybeError {

		if let Some(is_used) = self.subpool.get(incoming_texture) {
			// println!("(re-request) checking {incoming_texture:?} for being used before");
			assert!(is_used);
			// println!("(re-request) doing re-request for {incoming_texture:?}");
			texture_pool.remake_texture(texture_creation_info, incoming_texture, maybe_remake_transition_info)
		}
		else {
			panic!("Slot was not previously allocated in subpool!");
		}
	}

	// TODO: would making the incoming texture `mut` stop further usage of it?
	fn give_back_slot(&mut self, incoming_texture: &TextureHandle) {
		if let Some(is_used) = self.subpool.get_mut(incoming_texture) {
			// println!("(give back) checking {incoming_texture:?} for being used before");
			assert!(*is_used);
			// println!("(give back) setting {incoming_texture:?} to unused");
			*is_used = false;
		}
		else {
			panic!("Incoming texture did not already exist in subpool!");
		}
	}
}

//////////

type MessageID = Arc<str>;

enum SyncedMessageMapAction<'a, V, OffshoreV> {
	ExpireLocal(&'a V),
	MaybeUpdateLocal(&'a mut V, &'a OffshoreV),
	MakeLocalFromOffshore(&'a OffshoreV)
}

/* This is a utility type used for synchronizing
message info maps with other such maps. */
#[derive(Clone)]
struct SyncedMessageMap<V> {
	map: HashMap<MessageID, V>
}

impl<V> SyncedMessageMap<V> {
	fn new(max_size: usize) -> Self {
		Self {map: HashMap::with_capacity(max_size)}
	}

	fn from(map: HashMap<MessageID, V>, max_size: usize) -> Self {
		assert!(map.len() <= max_size);
		Self {map}
	}

	fn sync<OffshoreV>(&mut self, max_size: usize,
		offshore_map: &SyncedMessageMap<OffshoreV>,

		// TODO: make the output an enum too (would that be a dependent type?); perhaps via a mutable output parameter
		mut syncer: impl FnMut(SyncedMessageMapAction<'_, V, OffshoreV>) -> GenericResult<Option<V>>) -> MaybeError {

		let local = &mut self.map;
		let offshore = &offshore_map.map;

		// 1. Removing local ones that are not in the offshore
		local.retain(|local_key, local_value| {
			let keep_local_key = offshore.contains_key(local_key);
			if !keep_local_key {syncer(SyncedMessageMapAction::ExpireLocal(local_value)).unwrap();}
			keep_local_key
		});

		for (offshore_key, offshore_value) in offshore {
			if let Some(local_value) = local.get_mut(offshore_key) {
				// 2. If there's a local value already in the ofshore, update it
				syncer(SyncedMessageMapAction::MaybeUpdateLocal(local_value, offshore_value))?;
			}
			else {
				// 3. Otherwise, adding local ones that are not in the offshore
				let as_local_value = syncer(SyncedMessageMapAction::MakeLocalFromOffshore(offshore_value))?.unwrap();
				local.insert(offshore_key.clone(), as_local_value);
			}
		}

		////////// Doing a size assertion (mostly just to check that everything is working)

		let local_len = local.len();

		assert!(local_len <= max_size);
		assert!(local_len == offshore.len());

		////////// Returning

		Ok(())
	}
}

//////////

type MessageAgeData = Option<(&'static str, &'static str, i64)>;

// TODO: should/could I include caller ID, and an image, if sent?
#[derive(Clone)]
struct MessageInfo {
	age_data: MessageAgeData,
	display_text: String,
	maybe_from: Option<String>, // This is `None` if the message identity is hidden
	body: String,
	num_attachments: u32, // The amount of media attachments sent to the message
	time_sent: ReferenceTimestamp,
	time_loaded_by_app: ReferenceTimestamp, // This includes sub-second precision, while the time sent above does not
	just_updated: bool
}

struct ImmutableTwilioStateData {
	account_sid: String,
	request_auth: String,
	max_num_messages_in_history: usize,
	message_history_duration: Duration,
	reveal_texter_identities: bool
}

#[derive(Clone)]
struct TwilioStateData {
	// Immutable fields (in an `Arc` so they are not needlessly copied during the continual updating):
	immutable: Arc<ImmutableTwilioStateData>,

	// Mutable fields:
	curr_messages: SyncedMessageMap<MessageInfo>,
	unformatted_phone_number: String,
	formatted_phone_number_with_title: String
}

// TODO: put the non-continually-updated fields in their own struct
pub struct TwilioState {
	continually_updated: ContinuallyUpdated<TwilioStateData>,

	/* This is not continually updated because the text history windows need to
	be able to modify it directly. That is not possible with continually updated
	objects, because once their internal task finishes its work, any modifications
	made by the creator of the continually updated object will be overwritten with all
	newly computed data. */
	texture_subpool_manager: TextureSubpoolManager,
	id_to_texture_map: SyncedMessageMap<TextureHandle>, // TODO: integrate the subpool manager into this with the searching operations
	historically_sorted_messages_by_id: Vec<MessageID>, // TODO: avoid resorting with smart insertions and deletions?
	maybe_remake_transition_info: Option<RemakeTransitionInfo>
}

//////////

impl TwilioStateData {
	async fn new(account_sid: &str, auth_token: &str,
		max_num_messages_in_history: usize,
		message_history_duration: Duration,
		reveal_texter_identities: bool) -> GenericResult<Self> {

		use base64::{engine::general_purpose::STANDARD, Engine};
		let request_auth_base64 = STANDARD.encode(format!("{account_sid}:{auth_token}"));

		let mut data = Self {
			immutable: Arc::new(ImmutableTwilioStateData {
				account_sid: account_sid.to_owned(),
				request_auth: "Basic ".to_owned() + &request_auth_base64,
				max_num_messages_in_history,
				message_history_duration,
				reveal_texter_identities
			}),

			curr_messages: SyncedMessageMap::new(max_num_messages_in_history),
			unformatted_phone_number: String::new(),
			formatted_phone_number_with_title: String::new()
		};

		////////// Finding the phone number

		let json = data.do_twilio_request("IncomingPhoneNumbers", &[], &[]).await?;

		let Some(phone_numbers) = json["incoming_phone_numbers"].as_array()
		else {panic!("Expected the Twilio phone numbers to be an array!");};

		assert!(phone_numbers.len() == 1);

		let number = phone_numbers[0]["phone_number"].as_str().expect("Expected the phone number to be a string!");
		data.unformatted_phone_number = number.to_owned();
		data.formatted_phone_number_with_title = TwilioStateData::format_phone_number(number, "Messages to ", ":", "");

		//////////

		Ok(data)
	}

	async fn do_twilio_request(&self, endpoint: &str, path_params: &[Cow<'_, str>], query_params: &[(&str, Cow<'_, str>)]) -> GenericResult<serde_json::Value> {
		let base_url = format!("https://api.twilio.com/2010-04-01/Accounts/{}/{endpoint}.json", self.immutable.account_sid);
		let request_url = request::build_url(&base_url, path_params, query_params);

		request::as_type(request::get_with_maybe_header(
			&request_url, // TODO: cache the constructed requests, and why is there a 11200 error in the response for messages?
			Some(("Authorization", &self.immutable.request_auth))
		)).await
	}

	//////////

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

impl Updatable for TwilioStateData {
	type Param = ();

	async fn update(&mut self, _: &Self::Param) -> MaybeError {
		////////// Making a request, and getting a response

		let curr_time = ReferenceTimezone::now();
		let history_cutoff_time = curr_time - self.immutable.message_history_duration;
		let history_cutoff_day = history_cutoff_time.format("%Y-%m-%d");

		/* TODO:
		- Should I really limit the page size here? Twilio not returning messages in order might make this a problem...
		- When messages are sent with very small time gaps between each other, they can end up out of order - how to resolve? And is this a synchronization issue?
		*/

		let max_messages = self.immutable.max_num_messages_in_history;

		let json = self.do_twilio_request("Messages", &[],
			&[
				("To", Cow::Borrowed(self.unformatted_phone_number.as_str())), // Adding this filters out all outbound messages
				("PageSize", Cow::Borrowed(&max_messages.to_string())),
				("DateSent%3E", Cow::Borrowed(&history_cutoff_day.to_string())) // Note: the '%3E' is a URL-encoded '>'
			]
		).await?;

		////////// Creating a map of incoming messages

		// This will always be in the range of 0 <= num_messages <= self.num_messages_in_history
		let json_messages = json["messages"].as_array().unwrap();

		let incoming_message_map = HashMap::from_iter(
			json_messages.iter().filter_map(|message| {
				let message_field = |name| message[name].as_str().unwrap();

				// Using the date created instead, since it is never null at the beginning (unlike the date sent)
				let unparsed_time_sent = message_field("date_created");
				let time_sent = parse_time_from_rfc2822(unparsed_time_sent).unwrap();

				if time_sent >= history_cutoff_time {
					let id = message_field("uri");

					// If a key on the heap already existed, reuse it
					let (id_on_heap, time_loaded_by_app) =
						if let Some((already_id, already_message)) = self.curr_messages.map.get_key_value(id) {
							(already_id.clone(), already_message.time_loaded_by_app)
						}
						else {
							(id.into(), ReferenceTimezone::now())
						};

					let maybe_from = if self.immutable.reveal_texter_identities {
						Some(message_field("from"))
					}
					else {
						None
					};

					let body = message_field("body");
					let num_attachments = message_field("num_media").parse().unwrap();

					Some((id_on_heap, (maybe_from, body, num_attachments, time_sent, time_loaded_by_app)))
				}
				else {
					None
				}
			})
		);

		//////////

		self.curr_messages.sync(
			max_messages,
			&SyncedMessageMap::from(incoming_message_map, max_messages),

			|action_type| {
				match action_type {
					SyncedMessageMapAction::ExpireLocal(_) => {},

					SyncedMessageMapAction::MaybeUpdateLocal(curr_message, _) => {
						// Only making a new string if the age data became expired
						let age_data = Self::get_message_age_data(curr_time, curr_message.time_sent);

						curr_message.just_updated = age_data != curr_message.age_data;

						if curr_message.just_updated {
							curr_message.display_text = Self::make_message_display_text(
								age_data, &curr_message.body, curr_message.num_attachments, curr_message.maybe_from.as_deref()
							);

							curr_message.age_data = age_data;
						}
					},

					SyncedMessageMapAction::MakeLocalFromOffshore((maybe_from, body, num_attachments, wrongly_typed_time_sent, time_loaded_by_app)) => {
						let time_sent = (*wrongly_typed_time_sent).into();
						let age_data = Self::get_message_age_data(curr_time, time_sent);

						let num_attachments = *num_attachments;
						let trimmed_body = body.trim().to_string();
						let boxed_maybe_from = maybe_from.map(|from| from.to_owned());

						return Ok(Some(MessageInfo {
							age_data,
							display_text: Self::make_message_display_text(age_data, &trimmed_body, num_attachments, *maybe_from),
							maybe_from: boxed_maybe_from,
							body: trimmed_body,
							num_attachments,
							time_sent,
							time_loaded_by_app: *time_loaded_by_app,
							just_updated: true
						}));
					}
				}

				Ok(None)
			}
		)
	}
}

/* TODO: eventually, integrate `new` into `Updatable`, and
reduce the boilerplate for the `Updatable` stuff in general */
impl TwilioState {
	pub async fn new(
		account_sid: &str, auth_token: &str,
		max_num_messages_in_history: usize,
		message_history_duration: Duration,
		reveal_texter_identities: bool,
		maybe_remake_transition_info: Option<RemakeTransitionInfo>) -> GenericResult<Self> {

		let data = TwilioStateData::new(
			account_sid, auth_token, max_num_messages_in_history,
			message_history_duration, reveal_texter_identities
		).await?;

		Ok(Self {
			continually_updated: ContinuallyUpdated::new(&data, &(), "Twilio").await,
			texture_subpool_manager: TextureSubpoolManager::new(max_num_messages_in_history),
			id_to_texture_map: SyncedMessageMap::new(max_num_messages_in_history),
			historically_sorted_messages_by_id: Vec::new(),
			maybe_remake_transition_info
		})
	}

	// This returns false if something failed with the continual updater.
	pub fn update(&mut self, error_state: &mut ErrorState,
		texture_pool: &mut TexturePool,
		pixel_area: (u32, u32), font_info: &FontInfo,
		text_color: ColorSDL) -> GenericResult<bool> {

		let continual_updater_succeeded = self.continually_updated.update(&(), error_state)?;
		let curr_continual_data = self.continually_updated.get_data();

		let local = &mut self.id_to_texture_map;
		let offshore = &curr_continual_data.curr_messages;

		let mut texture_creation_info = TextureCreationInfo::Text((
			Cow::Borrowed(font_info),

			TextDisplayInfo {
				text: DisplayText::new(""),
				color: text_color,
				pixel_area,
				scroll_easer: easing_fns::scroll::PAUSE_THEN_SCROLL_LEFT,
				scroll_speed_multiplier: 1.0
			}
		));

		local.sync(
			curr_continual_data.immutable.max_num_messages_in_history,
			offshore,

			|action_type| {
				let mut update_texture_creation_info = |offshore_message_info: &MessageInfo| {
					if let TextureCreationInfo::Text((_, ref mut text_display_info)) = &mut texture_creation_info {
						// println!(">>> Update texture display info");
						text_display_info.text = DisplayText::new(&offshore_message_info.display_text).with_padding("", " ")
					}
				};

				match action_type {
					SyncedMessageMapAction::ExpireLocal(local_texture) => {
						// println!(">>> Give texture slot back");
						self.texture_subpool_manager.give_back_slot(local_texture);
					},

					SyncedMessageMapAction::MaybeUpdateLocal(local_texture, offshore_message_info) => {
						if offshore_message_info.just_updated {
							// println!(">>> Update local texture");
							update_texture_creation_info(offshore_message_info);

							self.texture_subpool_manager.re_request_slot(
								local_texture, &texture_creation_info,
								self.maybe_remake_transition_info.as_ref(),
								texture_pool
							)?;
						}
					},

					SyncedMessageMapAction::MakeLocalFromOffshore(offshore_message_info) => {
						// println!(">>> Allocate texture from base slot");
						assert!(offshore_message_info.just_updated);
						update_texture_creation_info(offshore_message_info);

						return Ok(Some(self.texture_subpool_manager.request_slot(
							&texture_creation_info,
							self.maybe_remake_transition_info.as_ref(),
							texture_pool
						)?));
					}
				}

				Ok(None)
			}
		)?;

		////////// After the syncing, sorting the messages by their IDs, and doing an assertion

		self.historically_sorted_messages_by_id = offshore.map.keys().cloned().collect();

		self.historically_sorted_messages_by_id.sort_by(|m1_id, m2_id| {
			let (m1, m2) = (&offshore.map[m1_id], &offshore.map[m2_id]);

			// Note: the smallest unit of time in `time_sent` is seconds.
			match m1.time_sent.cmp(&m2.time_sent) {
				/* If the messages were sent within the same second, ordering issues can occur.
				When that happens, resort to basing the ordering on the time that it was loaded by the app
				(which corresponds to the order provided by Twilio). This is not fully reliable either
				(since Twilio has no ordering guarantee), but it serves as a more reliable fallback in general,
				and using this ordering seems to work for me in practice. */

				std::cmp::Ordering::Equal => m2.time_loaded_by_app.cmp(&m1.time_loaded_by_app),
				other => other
			}
		});

		assert!(self.historically_sorted_messages_by_id.len() == local.map.len());

		Ok(continual_updater_succeeded)
	}
}

//////////

pub fn make_twilio_window(
	twilio_state: &TwilioState,
	update_rate: UpdateRate,
	top_left: Vec2f, size: Vec2f,
	top_box_height: f64,
	top_box_contents: WindowContents,
	message_background_contents_text_crop_factor: Vec2f,
	overall_border_color: Option<ColorSDL>, text_color: ColorSDL,
	message_background_contents: WindowContents) -> Window {

	struct TwilioHistoryWindowState {
		message_index: usize,
		text_color: ColorSDL
	}

	////////// Making a series of history windows

	let max_num_messages_in_history = twilio_state.continually_updated.get_data().immutable.max_num_messages_in_history;

	fn history_updater_fn(params: WindowUpdaterParams) -> MaybeError {
		let individual_window_state = params.window.get_state::<TwilioHistoryWindowState>();
		let inner_shared_state = params.shared_window_state.get_mut::<SharedWindowState>();
		let twilio_state = &mut inner_shared_state.twilio_state;
		let message_index = individual_window_state.message_index;

		// Running the updating only for the first history subwindow
		if message_index == 0 {
			twilio_state.update(&mut inner_shared_state.error_state,
				params.texture_pool, params.area_drawn_to_screen, inner_shared_state.font_info,
				individual_window_state.text_color)?;
		}

		let sorted_message_ids = &twilio_state.historically_sorted_messages_by_id;

		// Then, possibly assigning a texture to the window contents
		if message_index < sorted_message_ids.len() {
			let message_id = &sorted_message_ids[individual_window_state.message_index];

			// If this condition is not met, that means that the created texture is still pending
			if let Some(message_texture) = twilio_state.id_to_texture_map.map.get(message_id) {
				*params.window.get_contents_mut() = WindowContents::Texture(message_texture.clone());
			}
			else {
				panic!("A message texture was not allocated when it should have been!");
			}
		}
		else {
			*params.window.get_contents_mut() = WindowContents::Nothing;
		}

		Ok(())
	}

	let (cropped_text_tl_in_history_window, cropped_text_size_in_history_window) = (
		message_background_contents_text_crop_factor * Vec2f::new_scalar(0.5),
		Vec2f::ONE - message_background_contents_text_crop_factor
	);

	let history_window_height = 1.0 / max_num_messages_in_history as f64;

	let all_subwindows = (0..max_num_messages_in_history).map(|i| {
		// Note: I can't directly put the background contents into the history windows since it's sized differently
		let history_window = Window::new(
			Some((history_updater_fn, update_rate)),
			DynamicOptional::new(TwilioHistoryWindowState {message_index: i, text_color}),
			WindowContents::Nothing,
			None,
			cropped_text_tl_in_history_window,
			cropped_text_size_in_history_window,
			None
		);

		// This is just the history window with the background contents
		let mut with_background_contents = Window::new(
			None,
			DynamicOptional::NONE,
			message_background_contents.clone(),
			None,
			Vec2f::new(0.0, i as f64 * history_window_height),
			Vec2f::new(1.0, history_window_height),
			Some(vec![history_window])
		);

		// Don't want to not stretch the message bubbles
		with_background_contents.set_aspect_ratio_correction_skipping(true);
		with_background_contents
	}).collect();

	//////////

	fn top_box_updater_fn(params: WindowUpdaterParams) -> MaybeError {
		let inner_shared_state = params.shared_window_state.get::<SharedWindowState>();
		let twilio_state = inner_shared_state.twilio_state.continually_updated.get_data();
		let text_color = *params.window.get_state::<ColorSDL>();

		let WindowContents::Many(many) = params.window.get_contents_mut()
		else {panic!("The top box for Twilio did not contain a vec of contents!");};

		if let WindowContents::Nothing = many[1] {
			let texture_creation_info = TextureCreationInfo::Text((
				Cow::Borrowed(inner_shared_state.font_info),

				TextDisplayInfo {
					text: DisplayText::new(&twilio_state.formatted_phone_number_with_title).with_padding(" ", ""),
					color: text_color,
					pixel_area: params.area_drawn_to_screen,
					scroll_easer: easing_fns::scroll::STAY_PUT,
					scroll_speed_multiplier: 1.0
				}
			));

			many[1] = WindowContents::make_texture_contents(&texture_creation_info, params.texture_pool)?;
		}

		Ok(())
	}

	//////////

	let top_box = Window::new(
		Some((top_box_updater_fn, update_rate)),
		DynamicOptional::new(text_color),
		WindowContents::Many(vec![top_box_contents, WindowContents::Nothing]),
		None,
		Vec2f::new(top_left.x(), top_left.y() - top_box_height),
		Vec2f::new(size.x(), top_box_height),
		None
	);

	// This just contains the history windows
	let history_window_container = Window::new(
		None,
		DynamicOptional::NONE,
		WindowContents::Nothing,
		overall_border_color,
		top_left,
		size,
		Some(all_subwindows)
	);

	Window::new(
		None,
		DynamicOptional::NONE,
		WindowContents::Nothing,
		None,
		Vec2f::ZERO,
		Vec2f::ONE,
		Some(vec![history_window_container, top_box])
	)
}
