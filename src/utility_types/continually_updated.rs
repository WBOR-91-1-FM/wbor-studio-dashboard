use tokio::sync::mpsc::{
	self,
	error::{TrySendError, TryRecvError}
};

use crate::{
	utility_types::generic_result::*,
	dashboard_defs::error::ErrorState
};

//////////

pub trait Updatable: Clone + Send {
	type Param: Clone + Send + Sync;
	fn update(&mut self, param: &Self::Param) -> impl std::future::Future<Output = MaybeError> + Send;
}

// The main purpose of this is to be able to call async functions in sync contexts without blocking.
pub struct ContinuallyUpdated<T: Updatable> {
	curr_data: T,
	param_sender: mpsc::Sender<T::Param>,
	data_receiver: mpsc::Receiver<Result<T, String>>,
	name: &'static str
}

//////////

/* TODO: implement an intermediate syncing process for a continually updated struct, somehow?
That would be neat, for getting updates quicker, in some way. Checkpoint syncing function?
And for that, sleep in increments of 1 second, perhaps (just as a test), to get mini-updates. */
impl<T: Updatable + 'static> ContinuallyUpdated<T> {
	fn handle_channel_error(name: &str, transfer_description: &str, err: impl std::fmt::Display, should_panic: bool) {
		let message = format!("Problem from '{name}' with {transfer_description} ({}): '{err}'",
			if should_panic {"should never happen!"} else {"probably harmless, at program shutdown"}
		);

		if should_panic {
			panic!("{message}");
		}
		else {
			log::warn!("{message}");
		}
	}

	pub async fn new(data: &T, initial_param: &T::Param, name: &'static str) -> Self {
		let (data_sender, data_receiver) = mpsc::channel(1);
		let (param_sender, mut param_receiver) = mpsc::channel(1);

		if let Err(err) = param_sender.send(initial_param.clone()).await {
			panic!("Could not pass an initial param to the continual updater: '{err}'");
		}

		let mut cloned_data = data.clone();

		tokio::task::spawn(async move {
			loop {
				let param = match param_receiver.recv().await {
					Some(inner_param) => inner_param,

					_ => {
						return; // Bottom message is not printed since it happens almost every time

						/*
						return Self::handle_channel_error( // Ending the task
							name, "receiving parameter from main thread", "channel has been closed", false
						);
						*/
					}
				};

				//////////

				let result = match cloned_data.update(&param).await {
					Ok(_) => Ok(cloned_data.clone()),
					Err(err) => Err(err.to_string())
				};

				//////////

				if let Err(err) = data_sender.send(result).await {
					return Self::handle_channel_error( // Ending the task
						name, "sending computed data back to the main thread", err, false
					);
				}
			}
		});

		Self {curr_data: data.clone(), param_sender, data_receiver, name}
	}

	// This allows the param receiver to move past its await point, and starts a new update iteration with a new param.
	fn run_new_update_iteration(&self, param: &T::Param) {
		let transfer_description = "sending a parameter to its task";

		match self.param_sender.try_send(param.clone()) {
			Ok(()) => {},

			Err(TrySendError::Full(_)) =>
				Self::handle_channel_error(
					self.name, transfer_description, "channel is full", true
				),

			Err(TrySendError::Closed(_)) =>
				Self::handle_channel_error(
					self.name, transfer_description, "channel is closed", true
				)
		}
	}

	// This returns false if a task failed to complete its operation on its current iteration.
	pub fn update(&mut self, param: &T::Param, error_state: &mut ErrorState) -> bool {
		let mut error: Option<String> = None;

		match self.data_receiver.try_recv() {
			Ok(Ok(new_data)) => {
				self.curr_data = new_data;
				self.run_new_update_iteration(param);
			}

			Ok(Err(err)) => error = Some(err),

			// Waiting for a response...
			Err(TryRecvError::Empty) => {}

			Err(TryRecvError::Disconnected) => {
				Self::handle_channel_error(
					self.name, "receiving computed data from its task",
					"channel became disconnected", true
				);
			}
		}

		if let Some(err) = error {
			error_state.report(self.name, &err);
			self.run_new_update_iteration(param);
			return false;
		}
		else {
			error_state.unreport(self.name);
		}

		true
	}

	pub const fn get_data(&self) -> &T {
		&self.curr_data
	}
}
