use std::thread;
use std::sync::mpsc;

use crate::utility_types::generic_result::{GenericResult, MaybeError};

//////////

/* TODO:
- Can I avoid using threads here, and just have an async function that runs
in short bursts when you call `update` (or work with coroutines somehow)?
I can maybe use the `park_timeout`, `sleep`, `yield_now`, or `sleep_until` for that...

- Is there a way to keep the thread alive after finishing (continually sending),
and not doing channel reopening, thread restarting, and all of that? Perhaps put
the computation loop in the thread computer itself, possibly.

- Allow for thread joining (would be useful for Twilio) (but this is assuming that I don't do the thread-always-alive idea)
*/

pub trait Updatable {
	type Param: Clone + Send;
	fn update(&mut self, param: &Self::Param) -> MaybeError;
}

pub struct ContinuallyUpdated<T> {
	curr_data: T,
	thread_receiver: mpsc::Receiver<Result<T, String>>,
	name: &'static str
}

impl<T: Updatable + Clone + Send + 'static> ContinuallyUpdated<T> {
	/* TODO: can I make this lazy, so that it only starts working once I call `update`,
	and possibly only update again after a successful `update` call (with a pause?) */
	pub fn new(data: &T, param: <T as Updatable>::Param, name: &'static str) -> Self {
		let (thread_sender, thread_receiver) = mpsc::channel();

		let mut cloned_data = data.clone();

		let mut computer = move || {
			match cloned_data.update(&param) {
				Ok(_) => Ok(cloned_data.clone()),
				Err(err) => Err(err.to_string())
			}
		};

		thread::spawn(move || {
			if let Err(err) = thread_sender.send(computer()) {
				log::warn!("Problem with sending to thread (probably harmless): {err}");
			}
		});

		Self {curr_data: data.clone(), thread_receiver, name}
	}

	// This returns false if a thread failed to complete its operation.
	pub fn update(&mut self, param: <T as Updatable>::Param) -> GenericResult<bool> {
		let mut error: Option<String> = None;

		match self.thread_receiver.try_recv() {
			Ok(Ok(new_data)) => *self = Self::new(&new_data, param.clone(), self.name),
			Ok(Err(err)) => error = Some(err.into()),
			Err(mpsc::TryRecvError::Empty) => {}, // Waiting for a response...
			Err(err) => error = Some(err.to_string())
		}

		if let Some(err) = error {
			log::error!("Updating the {} data on this iteration failed. Error: '{err}'.", self.name);
			*self = Self::new(&self.curr_data, param.clone(), self.name); // Restarting when an error happens
			return Ok(false);
		}

		Ok(true)
	}

	pub const fn get_data(&self) -> &T {
		&self.curr_data
	}
}
