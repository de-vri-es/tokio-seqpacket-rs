use assert2::assert;
use tempfile::tempdir;
use tokio_seqpacket::{UnixSeqpacket, UnixSeqpacketListener};

/// Test that we can accept connections on the listener.
#[tokio::test]
async fn unix_seqpacket_listener() {
	let dir = tempdir().unwrap();
	let path = dir.path().join("listener.sock");

	let server_task = tokio::spawn({
		assert!(let Ok(mut listener) = UnixSeqpacketListener::bind(&path));
		assert!(let Ok(local_address) = listener.local_addr());
		assert!(local_address == path);
		async move {
			for _ in 0..2 {
				assert!(let Ok(peer) = listener.accept().await);
				assert!(let Ok(_) = peer.send(b"Hello!").await);
				let mut buf = [0u8; 128];
				assert!(let Ok(msg_info) = peer.recv(&mut buf).await);
				assert!(&buf[..msg_info.bytes_read()] == b"Goodbye!");
			}
		}
	});

	for _ in 0..2 {
		assert!(let Ok(peer) = UnixSeqpacket::connect(&path).await);
		let mut buf = [0u8; 128];
		assert!(let Ok(msg_info) = peer.recv(&mut buf).await);
		assert!(&buf[..msg_info.bytes_read()] == b"Hello!");
		assert!(let Ok(_) = peer.send(b"Goodbye!").await);
	}

	assert!(let Ok(()) = server_task.await);
}
