use assert2::{assert, let_assert};
use tokio_seqpacket::UnixSeqpacket;

/// Test a simple send and recv call.
#[tokio::test]
async fn send_recv() {
	let_assert!(Ok((mut a, mut b)) = UnixSeqpacket::pair());

	let (mut read_a, mut write_a) = a.split();
	let (mut read_b, mut write_b) = b.split();

	assert!(let Ok(_) = write_a.send(b"Hello B!").await);
	assert!(let Ok(_) = write_b.send(b"Hello A!").await);

	let mut buffer = [0u8; 128];

	let_assert!(Ok(len) = read_b.recv(&mut buffer).await);
	assert!(&buffer[..len] == b"Hello B!");

	let_assert!(Ok(len) = read_a.recv(&mut buffer).await);
	assert!(&buffer[..len] == b"Hello A!");
}
