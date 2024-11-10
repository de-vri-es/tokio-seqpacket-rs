#![cfg(any(target_os = "linux", target_os = "android"))]

use std::path::PathBuf;

use assert2::{assert, let_assert};
use tokio_seqpacket::{UnixSeqpacket, UnixSeqpacketListener};

#[track_caller]
fn random_abstract_name(suffix: &str) -> PathBuf {
	use std::io::Read;
	use std::ffi::OsString;
	use std::os::unix::ffi::OsStringExt;

	let_assert!(Ok(mut urandom) = std::fs::File::open("/dev/urandom"));
	let mut buffer = Vec::with_capacity(63 + suffix.len());
	buffer.resize(63, 0);
	assert!(let Ok(()) = urandom.read_exact(&mut buffer[1..]));
	for byte in &mut buffer[1..] {
		let c = *byte % (10 + 26 + 26);
		if c < 10 {
			*byte = b'0' + c;
		} else if c < 10 + 26 {
			*byte = b'A' + c - 10;
		} else {
			*byte = b'a' + c - 10 - 26;
		}
	}
	buffer.extend(suffix.bytes());
	OsString::from_vec(buffer).into()
}

/// Create a listening socket with an abstract name, connect to it and exchange a message.
///
/// Use an abstract socket path without terminating null byte.
#[tokio::test]
async fn address_without_null_byte() {
	let name = random_abstract_name("\x01");
	assert!(name.as_os_str().as_encoded_bytes().ends_with(&[1]), "{name:?}");

	let_assert!(Ok(mut listener) = UnixSeqpacketListener::bind(&name));
	let_assert!(Ok(local_addr) = listener.local_addr());
	assert!(local_addr == name);

	let (server_socket, client_socket) = tokio::join!(
		listener.accept(),
		UnixSeqpacket::connect(name),
	);
	let_assert!(Ok(server_socket) = server_socket);
	let_assert!(Ok(client_socket) = client_socket);

	assert!(let Ok(12) = client_socket.send(b"Hello world!").await);

	let mut buffer = [0u8; 128];
	assert!(let Ok(12) = server_socket.recv(&mut buffer).await);
	assert!(&buffer[..12] == b"Hello world!");
}

/// Create a listening socket with an abstract name, connect to it and exchange a message.
///
/// Use an abstract socket path with terminating null byte.
#[tokio::test]
async fn address_ending_with_null_byte() {
	let name = random_abstract_name("\x00");
	assert!(name.as_os_str().as_encoded_bytes().ends_with(&[0]), "{name:?}");

	let_assert!(Ok(mut listener) = UnixSeqpacketListener::bind(&name));
	let_assert!(Ok(local_addr) = listener.local_addr());
	assert!(local_addr == name);

	let (server_socket, client_socket) = tokio::join!(
		listener.accept(),
		UnixSeqpacket::connect(name),
	);
	let_assert!(Ok(server_socket) = server_socket);
	let_assert!(Ok(client_socket) = client_socket);

	assert!(let Ok(12) = client_socket.send(b"Hello world!").await);

	let mut buffer = [0u8; 128];
	assert!(let Ok(12) = server_socket.recv(&mut buffer).await);
	assert!(&buffer[..12] == b"Hello world!");
}
