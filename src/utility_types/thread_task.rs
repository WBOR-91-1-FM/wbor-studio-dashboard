use std::thread;
use std::sync::mpsc;
use crate::utility_types::generic_result::{self, {GenericResult, MaybeError}, SendableGenericResult};

struct ThreadTask<T> {
	thread_receiver: mpsc::Receiver<T>
}

impl<T: Send + 'static> ThreadTask<T> {
	fn new(mut computer: impl FnMut() -> T + Send + 'static) -> Self {
		let (thread_sender, thread_receiver) = mpsc::channel();

		thread::spawn(move || {
			if let Err(err) = thread_sender.send(computer()) {
				log::warn!("Problem with sending to thread (probably harmless): {err}");
			}
		});

		Self {thread_receiver}
	}

	fn get_value(&self) -> GenericResult<Option<T>> {
		match self.thread_receiver.try_recv() {
			Ok(value) => Ok(Some(value)),
			Err(mpsc::TryRecvError::Empty) => Ok(None),
			Err(err) => Err(err.into())
		}
	}
}

//////////

// TODO: can I avoid using threads here, and just have an async function that runs in short bursts when you call `update`?
pub trait Updatable {
	fn update(&mut self) -> MaybeError;
}

pub struct ContinuallyUpdated<T> {
	curr_data: T,
	update_task: ThreadTask<SendableGenericResult<T>>,
	name: &'static str
}

// TODO: inline `ThreadTask` into this?
impl<T: Updatable + Clone + Send + 'static> ContinuallyUpdated<T> {
	/* TODO: can I make this lazy, so that it only starts working once I call `update`,
	and possibly only update again after a successful `update` call (with a pause?) */
	pub fn new(data: &T, name: &'static str) -> Self {
		let mut cloned_data = data.clone();

		let update_task = ThreadTask::new(
			move || {
				generic_result::make_sendable(cloned_data.update())?;
				Ok(cloned_data.clone())
			}
		);

		Self {curr_data: data.clone(), update_task, name}
	}

	// This returns false if a thread failed to complete its operation.
	pub fn update(&mut self) -> GenericResult<bool> {
		let mut error: Option<Box<dyn std::error::Error>> = None;

		match self.update_task.get_value() {
			Ok(Some(result_or_err)) => {
				match result_or_err {
					Ok(result) => {*self = Self::new(&result, self.name);}
					Err(err) => {error = Some(err.into());}
				}
			},

			Ok(None) => {},
			Err(err) => {error = Some(err);}
		}

		if let Some(err) = error {
			log::error!("Updating the {} data on this iteration failed. Error: '{err}'.", self.name);
			*self = Self::new(&self.curr_data, self.name); // Restarting when an error happens
			return Ok(false);
		}

		Ok(true)
	}

	pub fn get_data(&self) -> &T {
		&self.curr_data
	}
}
