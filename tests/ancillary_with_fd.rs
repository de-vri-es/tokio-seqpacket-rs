use assert2::{assert, let_assert};
use std::io::{IoSlice, IoSliceMut, Read, Seek, Write};
use std::os::unix::io::{AsRawFd, FromRawFd};
use tempfile::tempfile;
use tokio_seqpacket::ancillary::{AncillaryData, SocketAncillary};
use tokio_seqpacket::{UnixSeqpacket};

#[tokio::test]
async fn pass_fd() {
	let_assert!(Ok(mut file) = tempfile());
	assert!(let Ok(_) = file.write_all(b"Wie dit leest is gek."));
	assert!(let Ok(0) = file.seek(std::io::SeekFrom::Start(0)));

	let_assert!(Ok((a, b)) = UnixSeqpacket::pair());

	let mut cmsg = [0; 64];
	let mut cmsg = SocketAncillary::new(&mut cmsg);
	cmsg.add_fds(&[file.as_raw_fd()]);

	assert!(let Ok(29) = a.send_vectored_with_ancillary(&[IoSlice::new(b"Here, have a file descriptor.")], &mut cmsg).await);
	drop(file);

	let mut cmsg = [0; 64];
	let mut cmsg = SocketAncillary::new(&mut cmsg);
	let mut read_buf = [0u8; 64];
	assert!(let Ok(29) = b.recv_vectored_with_ancillary(&mut [IoSliceMut::new(&mut read_buf)], &mut cmsg).await);
	assert!(&read_buf[..29] == b"Here, have a file descriptor.");

	let mut cmsgs = cmsg.messages();
	let_assert!(Some(Ok(AncillaryData::ScmRights(mut fds))) = cmsgs.next());
	assert!(let None = cmsgs.next());

	let_assert!(Some(fd) = fds.next());
	assert!(let None = fds.next());

	let mut file = unsafe { std::fs::File::from_raw_fd(fd) };
	let mut contents = Vec::new();
	assert!(let Ok(_) = file.read_to_end(&mut contents));
	assert!(contents == b"Wie dit leest is gek.");
}
