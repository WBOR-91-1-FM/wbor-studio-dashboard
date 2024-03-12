use chrono::DateTime;
use std::{borrow::Cow, collections::HashMap};

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

#[derive(Clone)]
enum TextureSubpoolHandle {
	Unallocated,
	AllocatedAndUnused(TextureHandle),
	AllocatedAndUsed(TextureHandle)
}

// TODO: perhaps use a hash table as the internal data structure here?
struct TextureSubpoolManager {
	subpool: Vec<TextureSubpoolHandle>
}

// TODO: add a fallback for all these textures, in case anything fails
impl TextureSubpoolManager {
	fn new(subpool_size: usize) -> Self {
		Self {subpool: vec![TextureSubpoolHandle::Unallocated; subpool_size]}
	}

	// This should be considered private
	fn check_for_no_match(texture: &TextureHandle, incoming_texture: &TextureHandle) {
		if texture == incoming_texture {
			panic!("Encountered impossible situation with texture subpool manager!");
		}
	}

	// TODO: can I bind the lifetime of each returned handle to the lifetime of the subpool manager?
	fn request_slot(&mut self, texture_creation_info: &TextureCreationInfo,
		texture_pool: &mut TexturePool) -> GenericResult<TextureHandle> {

		for wrapped_texture in &mut self.subpool {
			match wrapped_texture {
				// Allocating it, marking it as such + used, and returning it
				TextureSubpoolHandle::Unallocated => {
					// println!("- Creating a texture from scratch.");

					let texture = texture_pool.make_texture(texture_creation_info)?;
					*wrapped_texture = TextureSubpoolHandle::AllocatedAndUsed(texture.clone());
					return Ok(texture);
				},

				// Marking it as allocated and used, and returning it
				TextureSubpoolHandle::AllocatedAndUnused(texture) => {
					// println!("- Texture remake time!");

					texture_pool.remake_texture(texture_creation_info, texture)?;
					let cloned_texture = texture.clone();
					*wrapped_texture = TextureSubpoolHandle::AllocatedAndUsed(texture.clone());
					return Ok(cloned_texture);
				},

				TextureSubpoolHandle::AllocatedAndUsed(_) => continue
			}
		}

		panic!("No textures available for requesting in subpool!");
	}

	fn re_request_slot(&mut self, incoming_texture: &TextureHandle,
		texture_creation_info: &TextureCreationInfo, texture_pool: &mut TexturePool) -> GenericResult<()> {

		for wrapped_texture in &mut self.subpool {
			match wrapped_texture {
				TextureSubpoolHandle::Unallocated => continue,
				TextureSubpoolHandle::AllocatedAndUnused(texture) => Self::check_for_no_match(texture, incoming_texture),

				TextureSubpoolHandle::AllocatedAndUsed(texture) => {
					if texture == incoming_texture {
						return texture_pool.remake_texture(texture_creation_info, texture);
					}
				},
			}
		}

		panic!("Texture requested for remaking does not exist in subpool!");
	}

	// TODO: will making the incoming texture `mut` stop further usage of it?
	fn give_back_slot(&mut self, incoming_texture: &TextureHandle) {
		for wrapped_texture in &mut self.subpool {
			match wrapped_texture {
				TextureSubpoolHandle::Unallocated => continue,
				TextureSubpoolHandle::AllocatedAndUnused(texture) => Self::check_for_no_match(texture, incoming_texture),

				TextureSubpoolHandle::AllocatedAndUsed(texture) => {
					if texture == incoming_texture {
						*wrapped_texture = TextureSubpoolHandle::AllocatedAndUnused(texture.clone());
						return;
					}
				}
			}
		}

		panic!("Cannot give back texture! All textures have already been given back for subpool.");
	}
}

//////////

type MessageID = std::sync::Arc<str>;

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
		let mut map = HashMap::new();
		map.reserve(max_size);
		Self {map}
	}

	fn from(map: HashMap<MessageID, V>, max_size: usize) -> Self {
		assert!(map.len() <= max_size);
		Self {map}
	}

	fn sync<OffshoreV>(&mut self,
		max_size: usize,
		offshore_map: &SyncedMessageMap<OffshoreV>,
		// TODO: make the output an enum too (would that be a dependent type?)
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
	just_updated: bool // TODO: possibly remove later
}

// TODO: should I put all the never-mutated fields around an `Arc`?
#[derive(Clone)]
struct TwilioStateData {
	// Immutable fields:
	account_sid: String,
	request_auth: String,
	max_num_messages_in_history: usize,
	message_history_duration: chrono::Duration,

	// Mutable fields:
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
		let request_auth_base64 = STANDARD.encode(format!("{}:{}", account_sid, auth_token));

		Self {
			account_sid: account_sid.to_string(),
			request_auth: "Basic ".to_string() + &request_auth_base64,
			max_num_messages_in_history,
			message_history_duration,
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
			format!("{} {}{} ago: '{}'. ", unit_amount, unit_name, plural_suffix, body)
		}
		else {
			format!("Right now: '{}'. ", body)
		}
	}
}

impl Updatable for TwilioStateData {
	fn update(&mut self) -> GenericResult<()> {
		////////// Making a request, and getting a response

		let curr_time = Timezone::now();
		let history_cutoff_time = curr_time - self.message_history_duration;
		let history_cutoff_day = history_cutoff_time.format("%Y-%m-%d");

		// TODO: should I really limit the page size here? Twilio not returning messages in order might make this a problem...
		let base_url = format!("https://api.twilio.com/2010-04-01/Accounts/{}/Messages.json", self.account_sid);

		/* TODO: when messages are sent with very small time gaps between each other,
		they can end up out of order - how to resolve? And is this a synchronization issue? */
		let request_url = request::build_url(
			&base_url,
			&[],

			&[
				("PageSize", self.max_num_messages_in_history.to_string()),
				("DateSent%3E", history_cutoff_day.to_string()) // Note: the '%3E' is a URL-encoded '>'
			]
		)?;

		let response = request::get_with_maybe_header(
			&request_url, // TODO: cache the request, and why is there a 11200 error in the response?
			Some(("Authorization", &self.request_auth))
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
					let id = message_field("uri");

					// If a key on the heap already existed, reuse it
					let id_on_heap =
						if let Some((already_id, _)) = self.curr_messages.map.get_key_value(id) {already_id.clone()}
						else {id.into()};

					Some((id_on_heap, (message_field("from"), message_field("body"), time_sent)))
				}
				else {None}
			})
		);

		//////////

		self.curr_messages.sync(
			self.max_num_messages_in_history,
			&SyncedMessageMap::from(incoming_message_map, self.max_num_messages_in_history),

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
			continually_updated: ContinuallyUpdated::new(&data),
			texture_subpool_manager: TextureSubpoolManager::new(max_num_messages_in_history),
			id_to_texture_map: SyncedMessageMap::new(max_num_messages_in_history),
			historically_sorted_messages_by_id: Vec::new(),
			text_texture_creation_info_cache: None
		}
	}

	pub fn update(&mut self, texture_pool: &mut TexturePool) -> GenericResult<()> {
		// TODO: change other instances of `if-let` to this form
		let Some(((max_pixel_width, pixel_height), font_info, text_color)) = &self.text_texture_creation_info_cache else {
			// println!("It has not been cached yet, so wait for the next iteration");
			return Ok(());
		};

		// TODO: when this fails, just log the error, and return (so try again next iteration) (and maybe display something on screen)
		if let Err(err) = self.continually_updated.update() {
			println!("There was an error with updating the Twilio data: '{}'. Skipping this Twilio iteration.", err);
			return Ok(());
		}

		let curr_continual_data = self.continually_updated.get_data();

		let local = &mut self.id_to_texture_map;
		let offshore = &curr_continual_data.curr_messages;

		let mut texture_creation_info = TextureCreationInfo::Text((
			font_info,

			TextDisplayInfo {
				text: Cow::Borrowed(""),
				color: *text_color,

				scroll_fn: |secs_since_unix_epoch| {
					let total_cycle_time = 8.0;
					let scroll_time_percent = 0.25;

					let wait_boundary = total_cycle_time * scroll_time_percent;
					let scroll_value = secs_since_unix_epoch % total_cycle_time;

					let scroll_fract = if scroll_value < wait_boundary {scroll_value / wait_boundary} else {0.0};
					(scroll_fract, true)
				},

				max_pixel_width: *max_pixel_width,
				pixel_height: *pixel_height
			}
		));

		local.sync(
			curr_continual_data.max_num_messages_in_history,
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

		// TODO: for textures, could I keep only an allocated-but-unused pile, and a counter for allocated-but-used (instead)?

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
	message_background_contents_text_crop_factor: Vec2f,
	overall_border_color: ColorSDL, text_color: ColorSDL,
	message_background_contents: WindowContents) -> Window {

	struct TwilioHistoryWindowState {
		message_index: usize,
		text_color: ColorSDL
	}

	////////// Making a series of history windows

	let max_num_messages_in_history = twilio_state.continually_updated.get_data().max_num_messages_in_history;

	fn history_updater_fn((window, _, shared_state, area_drawn_to_screen): WindowUpdaterParams) -> GenericResult<()> {
		let inner_shared_state = shared_state.get_inner_value_mut::<SharedWindowState>();
		let twilio_state = &mut inner_shared_state.twilio_state;
		let individual_window_state = window.get_state::<TwilioHistoryWindowState>();
		let sorted_message_ids= &twilio_state.historically_sorted_messages_by_id;

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

	// TODO: have a top bar saying 'text this number: <number>, and have them received here!'

	let (cropped_text_tl_in_history_window, cropped_text_size_in_history_window) = (
		message_background_contents_text_crop_factor * Vec2f::new_scalar(0.5),
		Vec2f::ONE - message_background_contents_text_crop_factor
	);

	let history_window_height = 1.0 / max_num_messages_in_history as f32;

	let history_windows = (0..max_num_messages_in_history).rev().map(|i| {
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

	// This just contains the history windows
	Window::new(
		None,
		DynamicOptional::NONE,
		WindowContents::Nothing,
		Some(overall_border_color),
		top_left,
		size,
		Some(history_windows)
	)
}
