use interprocess::local_socket::{
	ToFsName,
	GenericFilePath,
	ListenerOptions,
	traits::Listener,
	ListenerNonblockingMode,
	prelude::LocalSocketListener
};

use std::io::{BufRead, BufReader};

use crate::utils::generic_result::*;

//////////

const IPC_STRING_MAX_SIZE: usize = 64;

//////////

pub type IpcSocketListener = interprocess::local_socket::prelude::LocalSocketListener;

pub async fn make_ipc_socket_listener(unformatted_path: &str) -> GenericResult<LocalSocketListener> {
	let path = format!("/tmp/wbor_studio_dashboard_{unformatted_path}.sock");

	let socket_path_fs_name = path.clone().to_fs_name::<GenericFilePath>()?;
	let make_listener = || ListenerOptions::new().name(socket_path_fs_name.clone()).create_sync();

	let listener = match make_listener() {
		Ok(listener) => listener,

		Err(err) => {
			log::warn!("A previous socket was still around after a previous crash; removing it and making a new one.");
			tokio::fs::remove_file(path).await?;
			make_listener().unwrap_or_else(|_| panic!("Could not create a listener: '{err}'."))
		}
	};

	listener.set_nonblocking(ListenerNonblockingMode::Both)?;

	Ok(listener)
}

pub fn try_listening_to_ipc_socket(listener: &mut LocalSocketListener) -> Option<String> {
	/* TODO: include some error handling here (should I care
	about the "resource temporarily unavailable" thing?) */
	if let Some(Ok(stream)) = listener.next() {
		let mut buffer = String::with_capacity(IPC_STRING_MAX_SIZE);
		let mut reader = BufReader::new(stream);
		let _ = reader.read_line(&mut buffer);
		Some(buffer)
	}
	else {
		None
	}
}
