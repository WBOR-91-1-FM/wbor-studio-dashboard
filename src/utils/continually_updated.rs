use tokio::{
	time::timeout,
	sync::mpsc::{self, error::{TrySendError, TryRecvError}}
};

use crate::{
	dashboard_defs::error::ErrorState,

	utils::{
		time::*,
		generic_result::*
	}
};

//////////

pub trait ContinuallyUpdatable: Clone + Send {
	type Param: Clone + Send + Sync;

	fn update(&mut self, param: &Self::Param) -> impl std::future::Future<Output = MaybeError> + Send;
}

// The main purpose of this is to be able to call async functions in sync contexts without blocking.
pub struct ContinuallyUpdated<T: ContinuallyUpdatable> {
	curr_data: T,
	param_sender: mpsc::Sender<T::Param>,
	data_receiver: mpsc::Receiver<Result<T, String>>,
	wakeup_sender: mpsc::Sender<()>,
	name: &'static str
}

#[derive(PartialEq)]
pub enum ContinuallyUpdatedState {
	Pending,
	GotNewData
}

/*
Here's how `ContinuallyUpdated` works (it's a bit like the actor model):
- This type keeps track of some data of type `T`, called `curr_data`.
- On its first update iteration, it receives a parameter of type `T::Param`, and then runs a trait function `update` which updates `curr_data`, given this parameter.
- It then sends back the current copy of the data over a channel. If an error occurred, it just sends an error back.
- It then sleeps for some amount of time, to ensure that updates don't happen too often (unless it's woken up prematurely).

- On subsequent iterations, it receives parameters from a parameter channel, and then sends back results as expected.
- The parent task that uses this `ContinuallyUpdated` type calls another function called `update` (passing an update parameter).
- If the task had completed its operation successfully, the client-side copy of the data is updated; otherwise, an error is reported (it's almost like errors result in a state rollback that way).
- If the newest data is in the midst of being computed, this other `update` function will return a `Pending` state, which indicates to the caller of `update` that there's no new data yet.

Overall, this is a little bit like asking someone to complete tasks for you, given a task specification (constrained by the inner `update` function, and the parameter).
To not burn themselves out working, they take breaks once they finish the task (this is the `sleep` I mentioned), unless they're prematurely woken up.
Any failures in completing the task results in the previous copy of the current data being kept on the client side, and an error being reported.

Note: updating the data involves quite a bit of copying, so ensure that your type `T` uses something like `Arc` around types that are expensive to copy.
*/

//////////

/* TODO: implement an intermediate syncing process for a continually updated struct, somehow?
That would be neat, for getting updates quicker, in some way. Checkpoint syncing function?
And for that, sleep in increments of 1 second, perhaps (just as a test), to get mini-updates. */
impl<T: ContinuallyUpdatable + 'static> ContinuallyUpdated<T> {
	fn channel_error(name: &str, transfer_description: &str, err: impl std::fmt::Display, should_panic: bool) {
		let message = format!("Problem from '{name}' with {transfer_description} ({}): '{err}'",
			if should_panic {"should ideally never happen"} else {"probably harmless, at program shutdown"}
		);

		if should_panic {
			panic!("{message}");
		}
		else {
			log::warn!("{message}");
		}
	}

	pub fn new(mut data: T, mut param: T::Param, name: &'static str, min_time_between_updates: Duration) -> Self {
		let (data_sender, data_receiver) = mpsc::channel(1);
		let (param_sender, mut param_receiver) = mpsc::channel::<T::Param>(1);
		let (wakeup_sender, mut wakeup_receiver) = mpsc::channel(1);

		let cloned_data = data.clone();

		tokio::task::spawn(async move {
			let mut is_first_iteration = true;

			loop {
				let time_before = get_reference_time();

				////////// Updating the current param

				if is_first_iteration {
					is_first_iteration = false;
				}
				else {
					match param_receiver.recv().await {
						Some(inner_param) => {
							param = inner_param.clone();
						}

						None => {
							return Self::channel_error( // Ending the task
								name, "receiving parameter from main thread", "channel has been closed", false
							);
						}
					}
				};

				////////// Computing a result

				// TODO: can I roll back in a more efficient way, without another clone?
				let data_before_update = data.clone();

				let result = match data.update(&param).await {
					Ok(()) => Ok(data.clone()),

					Err(err) => {
						data = data_before_update; // TODO: would rollbacks be able to 'lock' something in an invalid state?
						Err(err.to_string())
					}
				};

				////////// Sending it back

				if let Err(err) = data_sender.send(result).await {
					return Self::channel_error( // Ending the task
						name, "sending computed data back to the main thread", err, false
					);
				}

				////////// Sleeping as much as is needed, unless woken up early

				// TODO: should I use `signed_duration_since` here instead?
				let time_to_complete = get_reference_time() - time_before;
				let signed_time_to_sleep = min_time_between_updates - time_to_complete;

				let wakeup_receiver_error = || Self::channel_error(name, "waking up its task", "channel is closed", false);

				if let Ok(time_to_sleep) = signed_time_to_sleep.to_std() {
					// First, flush the wakeup receiver
					match wakeup_receiver.try_recv() {
						Ok(()) => {} // Premature flush of wakeup channel

						Err(TryRecvError::Empty) => {} // No flushing needed

						Err(TryRecvError::Disconnected) => {
							return wakeup_receiver_error();
						}
					}

					// Then, sleep for the remaining time, unless a wakeup was sent
					match timeout(time_to_sleep, wakeup_receiver.recv()).await {
						Ok(Some(())) => {} // A wakeup was sent!

						Ok(None) => {
							return; // This happens quite often, so I'm leaving this out
							// return wakeup_receiver_error();
						}

						Err(_) => {} // Nap time finished
					}
				}
			}
		});

		Self {
			curr_data: cloned_data,
			param_sender,
			data_receiver,
			wakeup_sender,
			name
		}
	}

	// This allows the param receiver to move past its await point, and starts a new update iteration with a new param.
	fn run_new_update_iteration(&self, param: T::Param) {
		let transfer_description = "sending a parameter to its task";

		match self.param_sender.try_send(param.clone()) {
			Ok(()) => {},

			Err(TrySendError::Full(_)) =>
				Self::channel_error(self.name, transfer_description, "channel is full", true),

			Err(TrySendError::Closed(_)) =>
				Self::channel_error(self.name, transfer_description, "channel is closed", true)
		}
	}

	pub fn wake_up_if_sleeping(&self) {
		match self.wakeup_sender.try_send(()) {
			Ok(()) => {}, // A wakeup was sent successfully

			Err(TrySendError::Full(_)) => {} // A wakeup was already sent

			Err(TrySendError::Closed(_)) =>
				Self::channel_error(self.name, "waking up its task", "channel is closed", true)
		}
	}

	pub fn update(&mut self, param: T::Param, error_state: &mut ErrorState) -> ContinuallyUpdatedState {
		match self.data_receiver.try_recv() {
			Ok(Ok(new_data)) => {
				self.curr_data = new_data;
				error_state.unreport(self.name);
				self.run_new_update_iteration(param);
				ContinuallyUpdatedState::GotNewData
			}

			Ok(Err(err)) => {
				error_state.report(self.name, &err);
				self.run_new_update_iteration(param);

				// Still waiting for new data, even though we got something back (a failure)...
				ContinuallyUpdatedState::Pending
			}

			// Waiting for a response...
			Err(TryRecvError::Empty) => ContinuallyUpdatedState::Pending,

			Err(TryRecvError::Disconnected) => {
				Self::channel_error(self.name,
					"receiving computed data from its task",
					"channel became disconnected", true
				);

				unreachable!();
			}
		}
	}

	pub const fn get_curr_data(&self) -> &T {
		&self.curr_data
	}
}
