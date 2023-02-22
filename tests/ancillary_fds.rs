use assert2::{assert, let_assert};
use std::io::Read;
use tokio_seqpacket::ancillary::{AncillaryMessageReader, OwnedAncillaryMessage};

mod ancillary_fd_helper;
use ancillary_fd_helper::receive_file_descriptor;

#[tokio::test]
async fn pass_fd() {
	// Receive a file descriptor
	let mut cmsg = [0; 64];
	let mut cmsg = AncillaryMessageReader::new(&mut cmsg);
	let fd = receive_file_descriptor(&mut cmsg).await;

	// Check that we can retrieve the message from the attached file.
	let_assert!(Ok(fd) = fd.try_clone_to_owned());
	let mut file = std::fs::File::from(fd);
	let mut contents = Vec::new();
	assert!(let Ok(_) = file.read_to_end(&mut contents));
	assert!(contents == b"Wie dit leest is gek.");
}

#[tokio::test]
async fn can_take_ownership_of_received_fds() {
	// Receive a file descriptor
	let mut cmsg = [0; 64];
	let mut cmsg = AncillaryMessageReader::new(&mut cmsg);
	let _fd = receive_file_descriptor(&mut cmsg).await;

	// Take ownership of the file descriptors.
	let mut msgs = cmsg.into_messages();
	let_assert!(Some(OwnedAncillaryMessage::FileDescriptors(mut fds)) = msgs.next());
	let_assert!(None = msgs.next());
	assert!(fds.len() == 1);
	let_assert!(Some(fd) = fds.take_ownership(0));
	let_assert!(None = fds.take_ownership(0));

	// Check that we can retrieve the message from the attached file.
	let mut file = std::fs::File::from(fd);
	let mut contents = Vec::new();
	assert!(let Ok(_) = file.read_to_end(&mut contents));
	assert!(contents == b"Wie dit leest is gek.");
}
