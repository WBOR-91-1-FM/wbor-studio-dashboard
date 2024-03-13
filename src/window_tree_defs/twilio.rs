use chrono::DateTime;
use std::{sync::Arc, borrow::Cow, collections::HashMap};

use crate::{
	request,

	utility_types::{
		generic_result::GenericResult,
		dynamic_optional::DynamicOptional,
		update_rate::UpdateRate, vec2f::Vec2f,
		thread_task::{ContinuallyUpdated, Updatable}
	},

	window_tree_defs::shared_window_state::SharedWindowState,
	window_tree::{ColorSDL, Window, WindowContents, WindowUpdaterParams},
	texture::{FontInfo, TextDisplayInfo, TextureCreationInfo, TextureHandle, TexturePool}
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
		texture_pool: &mut TexturePool) -> GenericResult<TextureHandle> {

		assert!(self.subpool.len() <= self.max_size);

		// If this is the case, go and check for unused variants
		if self.subpool.len() == self.max_size {
			for (texture, is_used) in &mut self.subpool {
				if !*is_used {
					// println!("(request) doing re-request, and setting {:?} to used", texture);
					*is_used = true;
					texture_pool.remake_texture(texture_creation_info, texture)?;
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

			// println!("(request) setting {:?} to used", texture);

			Ok(texture)
		}
	}

	fn re_request_slot(&mut self,
		incoming_texture: &TextureHandle,
		texture_creation_info: &TextureCreationInfo,
		texture_pool: &mut TexturePool) -> GenericResult<()> {

		if let Some(is_used) = self.subpool.get(incoming_texture) {
			// println!("(re-request) checking {:?} for being used before", incoming_texture);
			assert!(is_used);
			// println!("(re-request) doing re-request for {:?}", incoming_texture);
			texture_pool.remake_texture(texture_creation_info, incoming_texture)
		}
		else {
			panic!("Slot was not previously allocated in subpool!");
		}
	}

	// TODO: will making the incoming texture `mut` stop further usage of it?
	fn give_back_slot(&mut self, incoming_texture: &TextureHandle) {
		if let Some(is_used) = self.subpool.get_mut(incoming_texture) {
			// println!("(give back) checking {:?} for being used before", incoming_texture);
			assert!(*is_used);
			// println!("(give back) setting {:?} to unused", incoming_texture);
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

	fn sync<OffshoreV>(&mut self,
		max_size: usize,
		offshore_map: &SyncedMessageMap<OffshoreV>,
		// TODO: make the output an enum too (would that be a dependent type?); perhaps via a mutable output parameter
		mut syncer: impl FnMut(SyncedMessageMapAction<'_, V, OffshoreV>) -> GenericResult<Option<V>>)

		-> GenericResult<()> {

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

type Timezone = chrono::Utc; // This should not be changed (Twilio uses UTC by default)
type Timestamp = chrono::DateTime<Timezone>; // It seems like local time works too!
type MessageAgeData = Option<(&'static str, &'static str, i64)>;

// TODO: include caller ID, and an image, if sent?
#[derive(Clone)]
struct MessageInfo {
	age_data: MessageAgeData,
	display_text: String,
	from: String,
	body: String,
	time_sent: Timestamp,
	just_updated: bool
}

struct ImmutableTwilioStateData {
	account_sid: String,
	request_auth: String,
	max_num_messages_in_history: usize,
	message_history_duration: chrono::Duration
}

#[derive(Clone)]
struct TwilioStateData {
	// Immutable fields (in an `Arc` so they are not needlessly copied during the continual updating):
	immutable: Arc<ImmutableTwilioStateData>,

	// Mutable fields:
	phone_number_to: Option<Arc<str>>,
	curr_messages: SyncedMessageMap<MessageInfo>
}

// TODO: put the non-continually-updated fields in their own struct
pub struct TwilioState<'a> {
	continually_updated: ContinuallyUpdated<TwilioStateData>,

	/* This is not continually updated because the text history windows need to
	be able to modify it directly. That is not possible with continually updated
	objects, because once their internal thread finishes its work, any modifications
	made by the creator of the continually updated object will be overwritten with all
	newly computed data. */
	texture_subpool_manager: TextureSubpoolManager,
	id_to_texture_map: SyncedMessageMap<TextureHandle>, // TODO: integrate the subpool manager into this with the searching operations
	historically_sorted_messages_by_id: Vec<MessageID>, // TODO: avoid resorting with smart insertions and deletions?
	text_texture_creation_info_cache: Option<((u32, u32), &'a FontInfo<'a>, ColorSDL)>
}

//////////

impl TwilioStateData {
	fn new(account_sid: &str, auth_token: &str,
		max_num_messages_in_history: usize,
		message_history_duration: chrono::Duration) -> Self {

		use base64::{engine::general_purpose::STANDARD, Engine};
		let request_auth_base64 = STANDARD.encode(format!("{account_sid}:{auth_token}"));

		Self {
			immutable: Arc::new(ImmutableTwilioStateData {
				account_sid: account_sid.to_string(),
				request_auth: "Basic ".to_string() + &request_auth_base64,
				max_num_messages_in_history,
				message_history_duration
			}),

			phone_number_to: None,
			curr_messages: SyncedMessageMap::new(max_num_messages_in_history)
		}
	}

	//////////

	fn get_message_age_data(curr_time: Timestamp, time_sent: Timestamp) -> MessageAgeData {
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

	fn make_message_display_text(age_data: MessageAgeData, body: &str) -> String {
		if let Some((unit_name, plural_suffix, unit_amount)) = age_data {
			format!("{unit_amount} {unit_name}{plural_suffix} ago: '{body}'. ")
		}
		else {
			format!("Right now: '{body}'. ")
		}
	}
}

impl Updatable for TwilioStateData {
	fn update(&mut self) -> GenericResult<()> {
		////////// Making a request, and getting a response

		let curr_time = Timezone::now();
		let history_cutoff_time = curr_time - self.immutable.message_history_duration;
		let history_cutoff_day = history_cutoff_time.format("%Y-%m-%d");

		// TODO: should I really limit the page size here? Twilio not returning messages in order might make this a problem...
		let base_url = format!("https://api.twilio.com/2010-04-01/Accounts/{}/Messages.json", self.immutable.account_sid);

		let max_messages = self.immutable.max_num_messages_in_history;

		/* TODO: when messages are sent with very small time gaps between each other,
		they can end up out of order - how to resolve? And is this a synchronization issue? */
		let request_url = request::build_url(
			&base_url,
			&[],

			&[
				("PageSize", max_messages.to_string()),
				("DateSent%3E", history_cutoff_day.to_string()) // Note: the '%3E' is a URL-encoded '>'
			]
		)?;

		let response = request::get_with_maybe_header(
			&request_url, // TODO: cache the request, and why is there a 11200 error in the response?
			Some(("Authorization", &self.immutable.request_auth))
		)?;

		////////// Creating a map of incoming messages

		let json: serde_json::Value = serde_json::from_str(response.as_str()?)?;

		// This will always be in the range of 0 <= num_messages <= self.num_messages_in_history
		let json_messages = json["messages"].as_array().unwrap();

		let incoming_message_map = HashMap::from_iter(
			json_messages.iter().filter_map(|message| {
				let message_field = |name| message[name].as_str().unwrap();

				// Using the date created instead, since it is never null at the beginning (unlike the date sent)
				let unparsed_time_sent = message_field("date_created");
				let time_sent = DateTime::parse_from_rfc2822(unparsed_time_sent).unwrap();

				// TODO: see that the manual date filtering logic works
				if time_sent >= history_cutoff_time {
					if self.phone_number_to.is_none() {
						self.phone_number_to = Some(Arc::from(message_field("to")));
					}

					let id = message_field("uri");

					// If a key on the heap already existed, reuse it
					let id_on_heap =
						if let Some((already_id, _)) = self.curr_messages.map.get_key_value(id) {already_id.clone()}
						else {id.into()};

					Some((id_on_heap, (message_field("from"), message_field("body"), time_sent)))
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
							curr_message.display_text = Self::make_message_display_text(age_data, &curr_message.body);
							curr_message.age_data = age_data;
						}
					},

					SyncedMessageMapAction::MakeLocalFromOffshore((from, body, wrongly_typed_time_sent)) => {
						let time_sent = (*wrongly_typed_time_sent).into();
						let age_data = Self::get_message_age_data(curr_time, time_sent);

						return Ok(Some(MessageInfo {
							age_data,
							display_text: Self::make_message_display_text(age_data, body),
							from: from.to_string(),
							body: body.to_string(),
							time_sent,
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
impl TwilioState<'_> {
	pub fn new(
		account_sid: &str, auth_token: &str,
		max_num_messages_in_history: usize,
		message_history_duration: chrono::Duration) -> Self {

		let data = TwilioStateData::new(
			account_sid, auth_token, max_num_messages_in_history,
			message_history_duration
		);

		Self {
			continually_updated: ContinuallyUpdated::new(&data, "Twilio"),
			texture_subpool_manager: TextureSubpoolManager::new(max_num_messages_in_history),
			id_to_texture_map: SyncedMessageMap::new(max_num_messages_in_history),
			historically_sorted_messages_by_id: Vec::new(),
			text_texture_creation_info_cache: None
		}
	}

	pub fn update(&mut self, texture_pool: &mut TexturePool) -> GenericResult<()> {
		// TODO: change other instances of `if-let` to this form
		let Some(((max_pixel_width, pixel_height), font_info, text_color)) = self.text_texture_creation_info_cache else {
			// println!("It has not been cached yet, so wait for the next iteration");
			return Ok(());
		};

		self.continually_updated.update()?;

		let curr_continual_data = self.continually_updated.get_data();

		let local = &mut self.id_to_texture_map;
		let offshore = &curr_continual_data.curr_messages;

		let mut texture_creation_info = TextureCreationInfo::Text((
			font_info,

			TextDisplayInfo {
				text: Cow::Borrowed(""),
				color: text_color,

				scroll_fn: |secs_since_unix_epoch| {
					let total_cycle_time = 4.0;
					let scroll_time_percent = 0.75;

					let wait_boundary = total_cycle_time * scroll_time_percent;
					let scroll_value = secs_since_unix_epoch % total_cycle_time;

					let scroll_fract = if scroll_value < wait_boundary {scroll_value / wait_boundary} else {0.0};
					(scroll_fract, true)
				},

				max_pixel_width,
				pixel_height
			}
		));

		local.sync(
			curr_continual_data.immutable.max_num_messages_in_history,
			offshore,

			|action_type| {
				let mut update_texture_creation_info = |offshore_message_info: &MessageInfo| {
					if let TextureCreationInfo::Text((_, ref mut text_display_info)) = &mut texture_creation_info {
						// println!(">>> Update texture display info");
						text_display_info.text = Cow::Owned(offshore_message_info.display_text.clone());
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
							self.texture_subpool_manager.re_request_slot(local_texture, &texture_creation_info, texture_pool)?;
						}
					},

					SyncedMessageMapAction::MakeLocalFromOffshore(offshore_message_info) => {
						// println!(">>> Allocate texture from base slot");
						assert!(offshore_message_info.just_updated);
						update_texture_creation_info(offshore_message_info);
						return Ok(Some(self.texture_subpool_manager.request_slot(&texture_creation_info, texture_pool)?));
					}
				}

				Ok(None)
			}
		)?;

		////////// After the syncing, sorting the messages by their IDs, and doing an assertion

		self.historically_sorted_messages_by_id = offshore.map.keys().cloned().collect();
		self.historically_sorted_messages_by_id.sort_by_key(|id| offshore.map[id].time_sent);
		assert!(self.historically_sorted_messages_by_id.len() == local.map.len());

		Ok(())
	}
}

//////////

pub fn make_twilio_window(
	twilio_state: &TwilioState,
	update_rate: UpdateRate,
	top_left: Vec2f, size: Vec2f,
	top_box_height: f32,
	top_box_contents: WindowContents,
	message_background_contents_text_crop_factor: Vec2f,
	overall_border_color: ColorSDL, text_color: ColorSDL,
	message_background_contents: WindowContents) -> Window {

	struct TwilioHistoryWindowState {
		message_index: usize,
		text_color: ColorSDL
	}

	////////// Making a series of history windows

	let max_num_messages_in_history = twilio_state.continually_updated.get_data().immutable.max_num_messages_in_history;

	fn history_updater_fn((window, _, shared_state, area_drawn_to_screen): WindowUpdaterParams) -> GenericResult<()> {
		let inner_shared_state = shared_state.get_inner_value_mut::<SharedWindowState>();
		let twilio_state = &mut inner_shared_state.twilio_state;
		let individual_window_state = window.get_state::<TwilioHistoryWindowState>();
		let sorted_message_ids = &twilio_state.historically_sorted_messages_by_id;

		// Filling the text texture creation info cache
		if twilio_state.text_texture_creation_info_cache.is_none() {
			twilio_state.text_texture_creation_info_cache = Some((
				(area_drawn_to_screen.width(), area_drawn_to_screen.height()),
				inner_shared_state.font_info,
				individual_window_state.text_color
			));
		}

		// Then, possibly assigning a texture to the window contents
		if individual_window_state.message_index < sorted_message_ids.len() {
			let message_id = &sorted_message_ids[individual_window_state.message_index];

			// If this condition is not met, that means that the created texture is still pending
			if let Some(message_texture) = twilio_state.id_to_texture_map.map.get(message_id) {
				*window.get_contents_mut() = WindowContents::Texture(message_texture.clone());
			}
			else {
				panic!("A message texture was not allocated when it should have been!");
			}
		}
		else {
			*window.get_contents_mut() = WindowContents::Nothing;
		}

		Ok(())
	}

	let (cropped_text_tl_in_history_window, cropped_text_size_in_history_window) = (
		message_background_contents_text_crop_factor * Vec2f::new_scalar(0.5),
		Vec2f::ONE - message_background_contents_text_crop_factor
	);

	let history_window_height = 1.0 / max_num_messages_in_history as f32;

	let all_subwindows = (0..max_num_messages_in_history).rev().map(|i| {
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
		Window::new(
			None,
			DynamicOptional::NONE,
			message_background_contents.clone(),
			None,
			Vec2f::new(0.0, history_window_height * i as f32),
			Vec2f::new(1.0, history_window_height),
			Some(vec![history_window])
		)
	}).collect();

	//////////

	fn top_box_updater_fn((window, texture_pool, shared_state, area_drawn_to_screen): WindowUpdaterParams) -> GenericResult<()> {
		let text_color: ColorSDL = *window.get_state();
		let inner_shared_state = shared_state.get_inner_value_mut::<SharedWindowState>();
		let twilio_state = inner_shared_state.twilio_state.continually_updated.get_data();

		if let Some(phone_number) = &twilio_state.phone_number_to {
			let WindowContents::Many(many) = window.get_contents_mut()
			else {panic!("The top box for Twilio did not contain a vec of contents!");};

			if let WindowContents::Nothing = many[1] {
				let texture_creation_info = TextureCreationInfo::Text((
					inner_shared_state.font_info,

					TextDisplayInfo {
						text: Cow::Owned(format!("Messages to {phone_number}:")),
						color: text_color,
						scroll_fn: |_| (0.0, true),
						max_pixel_width: area_drawn_to_screen.width(),
						pixel_height: area_drawn_to_screen.height()
					}
				));

				let text_texture = texture_pool.make_texture(&texture_creation_info)?;
				many[1] = WindowContents::Texture(text_texture);
			}
		}

		Ok(())
	}

	let top_box = Window::new(
		Some((top_box_updater_fn, update_rate)),
		DynamicOptional::new(text_color),
		WindowContents::Many(vec![top_box_contents, WindowContents::Nothing]),
		None,
		Vec2f::new(top_left.x(), top_left.y() - top_box_height),
		Vec2f::new(size.x(), top_box_height),
		None
	);

	//////////

	// This just contains the history windows
	let history_window_container = Window::new(
		None,
		DynamicOptional::NONE,
		WindowContents::Nothing,
		Some(overall_border_color),
		top_left,
		size,
		Some(all_subwindows)
	);

	Window::new(
		None,
		DynamicOptional::NONE,
		WindowContents::Nothing,
		Some(overall_border_color),
		Vec2f::ZERO,
		Vec2f::ONE,
		Some(vec![history_window_container, top_box])
	)
}
