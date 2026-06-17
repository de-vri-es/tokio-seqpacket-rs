use assert2::assert;
use std::io::{IoSlice, IoSliceMut, Read, Seek, Write};
use tempfile::tempfile;
use tokio_seqpacket::ancillary::{AncillaryMessageReader, AncillaryMessageWriter, OwnedAncillaryMessage};
use tokio_seqpacket::UnixSeqpacket;

pub async fn receive_file_descriptor(ancillary_buf: &mut [u8]) -> AncillaryMessageReader<'_> {
	let socket_b = {
		// Make a file to send as attachment.
		assert!(let Ok(mut file_a) = tempfile());
		assert!(let Ok(_) = file_a.write_all(b"Wie dit leest is gek."));
		assert!(let Ok(()) = file_a.rewind());

		assert!(let Ok(mut file_b) = tempfile());
		assert!(let Ok(_) = file_b.write_all(b"Wie dit leest is gek."));
		assert!(let Ok(()) = file_b.rewind());

		// Make a pair of connected sockets to send it over.
		assert!(let Ok((socket_a, socket_b)) = UnixSeqpacket::pair());

		// Prepare an ancillary message and add the file descriptor to it.
		let mut cmsg = [0; 64];
		let mut cmsg = AncillaryMessageWriter::new(&mut cmsg);
		assert!(let Ok(()) = cmsg.add_fds([&file_a, &file_b]));

		// Send the message with file descriptor.
		assert!(let Ok(29) = socket_a.send_vectored_with_ancillary(&[IoSlice::new(b"Here, have a file descriptor.")], &mut cmsg).await);

		// Return the receiving socket from the scope.
		socket_b
	};

	let mut read_buf = [0u8; 64];
	assert!(let Ok((msg_info, cmsg)) = socket_b.recv_vectored_with_ancillary(&mut [IoSliceMut::new(&mut read_buf)], ancillary_buf).await);
	assert!(msg_info.bytes_read() == 29);
	assert!(&read_buf[..29] == b"Here, have a file descriptor.");

	cmsg
}

#[tokio::test]
async fn can_take_ownership_of_received_fds() {
	// Receive a file descriptor
	let mut ancillary_buffer = [0; 64];
	let ancillary = receive_file_descriptor(&mut ancillary_buffer).await;

	// Take ownership of the file descriptors.
	let mut messages = ancillary.into_messages();
	assert!(let Some(OwnedAncillaryMessage::FileDescriptors(fds)) = messages.next());
	assert!(let None = messages.next());
	assert!(fds.len() == 2);

	let mut fds: Vec<_> = fds.collect();
	assert!(fds.len() == 2);

	// Check that we can retrieve the message from the attached file.
	let mut file_b = std::fs::File::from(fds.remove(1));
	let mut file_a = std::fs::File::from(fds.remove(0));

	let mut contents = Vec::new();
	assert!(let Ok(_) = file_a.read_to_end(&mut contents));
	assert!(contents == b"Wie dit leest is gek.");

	let mut contents = Vec::new();
	assert!(let Ok(_) = file_b.read_to_end(&mut contents));
	assert!(contents == b"Wie dit leest is gek.");
}
