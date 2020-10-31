use assert2::{assert, let_assert};
use tempfile::tempdir;
use tokio_seqpacket::{UnixSeqpacket, UnixSeqpacketListener};

/// Test that we can accept connections on the listener.
#[test]
fn unix_seqpacket_listener() {
	let dir = tempdir().unwrap();
	let listener = dir.path().join("listener.sock");

	let mut runtime = tokio::runtime::Builder::new_current_thread()
		.enable_all()
		.build()
		.unwrap();
	let local = tokio::task::LocalSet::new();

	local.block_on(&mut runtime, async move {
		let server_task = tokio::task::spawn_local({
			let_assert!(Ok(mut listener) = UnixSeqpacketListener::bind(&listener));
			async move {
				for _ in 0..2 {
					let_assert!(Ok((mut peer, addr)) = listener.accept().await);
					assert!(let None = addr.as_pathname());
					assert!(let Ok(_) = peer.send(b"Hello!").await);
					let mut buf = [0u8; 128];
					let_assert!(Ok(len) = peer.recv(&mut buf).await);
					assert!(&buf[..len] == b"Goodbye!");
				}
			}
		});

		for _ in 0..2 {
			let_assert!(Ok(mut peer) = UnixSeqpacket::connect(&listener).await);
			let mut buf = [0u8; 128];
			let_assert!(Ok(len) = peer.recv(&mut buf).await);
			assert!(&buf[..len] == b"Hello!");
			assert!(let Ok(_) = peer.send(b"Goodbye!").await);
		}

		assert!(let Ok(()) = server_task.await);
	})
}
