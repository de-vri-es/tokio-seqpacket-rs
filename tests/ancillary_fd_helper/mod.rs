use assert2::assert;
use std::io::{IoSlice, IoSliceMut, Seek, Write};
use std::os::fd::AsFd;
use tempfile::tempfile;
use tokio_seqpacket::ancillary::{AncillaryMessageReader, AncillaryMessageWriter};
use tokio_seqpacket::UnixSeqpacket;

pub async fn receive_file_descriptor_socket(file_payload: &[u8], socket_payload: &[u8]) -> UnixSeqpacket {
	// Make a file to send as attachment.
	assert!(let Ok(mut file) = tempfile());
	assert!(let Ok(_) = file.write_all(file_payload));
	assert!(let Ok(()) = file.rewind());

	// Make a pair of connected sockets to send it over.
	assert!(let Ok((socket_a, socket_b)) = UnixSeqpacket::pair());

	// Prepare an ancillary message and add the file descriptor to it.
	let mut cmsg = [0; 64];
	let mut cmsg = AncillaryMessageWriter::new(&mut cmsg);
	assert!(let Ok(()) = cmsg.add_fds([file.as_fd()]));

	// Send the message with file descriptor.
	assert!(let Ok(n_written) = socket_a.send_vectored_with_ancillary(&[IoSlice::new(socket_payload)], &mut cmsg).await);
	assert_eq!(n_written, socket_payload.len());

	// Return the receiving socket from the scope.
	socket_b
}

// This function is unused in the `ancillary_fds_peek` test, but used by others.
#[allow(dead_code)]
pub async fn receive_file_descriptor(ancillary_buf: &mut [u8]) -> AncillaryMessageReader<'_> {
	let socket = receive_file_descriptor_socket(b"Wie dit leest is gek.", b"Here, have a file descriptor.").await;

	let mut read_buf = [0u8; 64];
	assert!(let Ok((msg_info, cmsg)) = socket.recv_vectored_with_ancillary(&mut [IoSliceMut::new(&mut read_buf)], ancillary_buf).await);
	assert_eq!(msg_info.bytes_read(), 29);
	assert!(&read_buf[..29] == b"Here, have a file descriptor.");

	cmsg
}
