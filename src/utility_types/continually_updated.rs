use std::sync::mpsc;

use crate::{
	utility_types::generic_result::*,
	dashboard_defs::error::ErrorState
};

// TODO: could I model this better as an infinite stream instead?

//////////

pub trait Updatable: Clone + Send {
	type Param: Clone + Send + Sync;
	fn update(&mut self, param: &Self::Param) -> impl std::future::Future<Output = MaybeError> + Send;
}

pub struct ContinuallyUpdated<T: Updatable> {
	curr_data: T,
	param_sender: mpsc::SyncSender<T::Param>,
	data_receiver: mpsc::Receiver<Result<T, String>>,
	name: &'static str
}

//////////

impl<T: Updatable + 'static> ContinuallyUpdated<T> {
	pub fn new(data: &T, initial_param: &T::Param, name: &'static str) -> Self {
		let (data_sender, data_receiver) = mpsc::sync_channel(1); // This can be async if needed
		let (param_sender, param_receiver) = mpsc::sync_channel(1);

		let mut cloned_data = data.clone();

		fn handle_channel_error<Error: std::fmt::Display>(err: Error, name: &str, transfer_description: &str) {
			log::warn!("Problem from '{name}' with {transfer_description} main thread (probably harmless, at program shutdown): '{err}'");
		}

		tokio::task::spawn(async move {
			loop {
				/* `recv` will block until it receives the parameter! The parameters will
				only be passed once the data has been received on the main thread. */
				let param = match param_receiver.recv() {
					Ok(inner_param) => inner_param,

					Err(_err) => {
						// This happens almost every time, so just silencing this (it's harmless)
						// handle_channel_error(_err, name, "receiving parameter from");
						return;
					}
				};

				let result = match cloned_data.update(&param).await {
					Ok(_) => Ok(cloned_data.clone()),
					Err(err) => Err(err.to_string())
				};

				if let Err(err) = data_sender.send(result) {
					handle_channel_error(err, name, "sending data back to the");
					return;
				}
			}
		});

		let continually_updated = Self {
			curr_data: data.clone(), param_sender,
			data_receiver, name
		};

		if let Err(err) = continually_updated.run_new_update_iteration(initial_param) {
			panic!("Could not pass an initial param to the continual updater: '{err}'");
		}

		continually_updated
	}

	// This unblocks the param receiver and starts a new update iteration with a new param
	fn run_new_update_iteration(&self, param: &T::Param) -> MaybeError {
		self.param_sender.send(param.clone()).to_generic()
	}

	// This returns false if a task failed to complete its operation on its current iteration.
	pub fn update(&mut self, param: &T::Param, error_state: &mut ErrorState) -> GenericResult<bool> {
		let mut error: Option<String> = None;

		match self.data_receiver.try_recv() {
			Ok(Ok(new_data)) => {
				self.curr_data = new_data;
				self.run_new_update_iteration(param)?;
			}

			Ok(Err(err)) => error = Some(err),

			// Waiting for a response...
			Err(mpsc::TryRecvError::Empty) => {}

			Err(err) => error = Some(err.to_string())
		}

		if let Some(err) = error {
			error_state.report(self.name, &err);
			self.run_new_update_iteration(param)?;
			return Ok(false);
		}
		else {
			error_state.unreport(self.name);
		}

		Ok(true)
	}

	pub const fn get_data(&self) -> &T {
		&self.curr_data
	}
}
