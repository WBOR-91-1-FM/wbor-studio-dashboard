use std::thread;
use std::sync::mpsc;
use crate::utility_types::generic_result::GenericResult;

pub struct ThreadTask<T> {
	thread_receiver: mpsc::Receiver<T>
}

impl<T: Send + 'static> ThreadTask<T> {
	pub fn new(mut computer: impl FnMut() -> T + Send + 'static) -> Self {
		let (thread_sender, thread_receiver) = mpsc::channel();

		thread::spawn(move || {
			thread_sender.send(computer()).unwrap()
		});

		Self {thread_receiver}
	}

	pub fn get_value(&self) -> GenericResult<Option<T>> {
		match self.thread_receiver.try_recv() {
			Ok(value) => Ok(Some(value)),
			Err(mpsc::TryRecvError::Empty) => Ok(None),
			Err(err) => Err(err.into())
		}
	}
}
