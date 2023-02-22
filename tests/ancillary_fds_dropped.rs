use assert2::{assert, let_assert};
use std::os::fd::{AsRawFd, OwnedFd, FromRawFd};
use tokio_seqpacket::ancillary::AncillaryMessageReader;

mod ancillary_fd_helper;
use ancillary_fd_helper::receive_file_descriptor;

#[tokio::test]
async fn dropping_ancilarry_drops_owned_fds() {
	// Receive a file descriptor
	let mut cmsg = [0; 64];
	let mut cmsg = AncillaryMessageReader::new(&mut cmsg);
	let fd = receive_file_descriptor(&mut cmsg).await;

	// Remember the raw fd so we can check it gets closed when we drop the `SocketAncillary`.
	let raw_fd = fd.as_raw_fd();

	// Drop the ancillary and check that the file descriptor is now invalid.
	// NOTE: this is technically a race condition:
	// another thread/test could re-use the fd between dropping the cmsg and trying to duplicate the fd.
	// That is why this test is in an integration test by itself.
	let owned_fd = std::mem::ManuallyDrop::new(unsafe { OwnedFd::from_raw_fd(raw_fd) });
	drop(cmsg);
	let_assert!(Err(e) = owned_fd.try_clone());
	assert!(e.raw_os_error() == Some(libc::EBADF));
}
