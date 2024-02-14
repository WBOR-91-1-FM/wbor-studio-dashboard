use std::thread;
use std::sync::mpsc;
use crate::utility_types::generic_result::{self, GenericResult, SendableGenericResult};

pub struct ThreadTask<T> {
	thread_receiver: mpsc::Receiver<T>
}

impl<T: Send + 'static> ThreadTask<T> {
	pub fn new(mut computer: impl FnMut() -> T + Send + 'static) -> Self {
		let (thread_sender, thread_receiver) = mpsc::channel();

		thread::spawn(move || {
			// TODO: fix the panics that occasionally happen here when exiting the app
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

//////////

pub trait Updatable {
	fn update(&mut self) -> GenericResult<()>;
}

pub struct ContinuallyUpdated<T> {
	data: T,
	update_task: ThreadTask<SendableGenericResult<T>>
}

impl<T: Updatable + Clone + Send + 'static> ContinuallyUpdated<T> {
	pub fn new(data: &T) -> Self {
		let mut cloned_data = data.clone();

		let update_task = ThreadTask::new(
			move || {
				generic_result::make_sendable(cloned_data.update())?;
				Ok(cloned_data.clone())
			}
		);

		Self {data: data.clone(), update_task}
	}

	pub fn update(&mut self) -> GenericResult<()> {
		if let Some(data) = self.update_task.get_value()? {
			self.data = data?; // TODO: remove this redundant set
			*self = Self::new(&self.data);
		}

		Ok(())
	}

	pub fn get_data(&self) -> &T {
		&self.data
	}
}
