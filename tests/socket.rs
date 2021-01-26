use assert2::{assert, let_assert};
use tokio_seqpacket::UnixSeqpacket;

/// Test a simple send and recv call.
#[tokio::test]
async fn send_recv() {
	let_assert!(Ok((mut a, mut b)) = UnixSeqpacket::pair());
	assert!(let Ok(12) = a.send(b"Hello world!").await);

	let mut buffer = [0u8; 128];
	assert!(let Ok(12) = b.recv(&mut buffer).await);
	assert!(&buffer[..12] == b"Hello world!");
}

/// Test a send and receive call where the send wakes the recv task.
#[test]
fn send_recv_out_of_order() {
	use std::sync::atomic::{AtomicBool, Ordering};

	let mut runtime = tokio::runtime::Builder::new_current_thread()
		.enable_all()
		.build()
		.unwrap();
	let local = tokio::task::LocalSet::new();

	local.block_on(&mut runtime, async {
		// Atomic bools to verify things happen in the order we want.
		// We're using a local task set to ensure we're single threaded.
		static ABOUT_TO_READ: AtomicBool = AtomicBool::new(false);

		let_assert!(Ok((mut a, mut b)) = UnixSeqpacket::pair());

		// Spawning a task shouldn't run anything until the current task awaits something.
		// Still, we use the atomic boolean to double-check that.
		let task = tokio::task::spawn_local(async move {
			assert!(ABOUT_TO_READ.load(Ordering::Relaxed) == true);
			assert!(let Ok(12) = a.send(b"Hello world!").await);
		});

		let mut buffer = [0u8; 128];
		ABOUT_TO_READ.store(true, Ordering::Relaxed);
		assert!(let Ok(12) = b.recv(&mut buffer).await);
		assert!(&buffer[..12] == b"Hello world!");

		assert!(let Ok(()) = task.await);
	});
}

/// Test a simple send_vectored and recv_vectored call.
#[tokio::test]
async fn send_recv_vectored() {
	use std::io::{IoSlice, IoSliceMut};

	let_assert!(Ok((mut a, mut b)) = UnixSeqpacket::pair());
	assert!(let Ok(12) = a.send_vectored(&[
		IoSlice::new(b"Hello"),
		IoSlice::new(b" "),
		IoSlice::new(b"world"),
		IoSlice::new(b"!"),
	]).await);

	let mut hello = [0u8; 5];
	let mut space = [0u8; 1];
	let mut world = [0u8; 5];
	let mut punct = [0u8; 1];
	assert!(let Ok(12) = b.recv_vectored(&mut [
		IoSliceMut::new(&mut hello),
		IoSliceMut::new(&mut space),
		IoSliceMut::new(&mut world),
		IoSliceMut::new(&mut punct),
	]).await);

	assert!(&hello == b"Hello");
	assert!(&space == b" ");
	assert!(&world == b"world");
	assert!(&punct == b"!");
}

#[test]
fn echo_loop() {
	let runtime = tokio::runtime::Builder::new_current_thread()
		.enable_all()
		.build()
		.unwrap();

	let (server_result, client_result) = runtime.block_on(async {
		let_assert!(Ok((client, server)) = UnixSeqpacket::pair());

		let server = tokio::task::spawn(async move {
			let mut buf = vec![0u8; 2048];
			loop {
				println!("waiting for next request");
				let n_received = server.recv(&mut buf).await.unwrap();
				println!("recvd: {}", String::from_utf8_lossy(&buf[..n_received]));
				if n_received == 0 {
					break;
				}
				server.send(&buf[..n_received]).await.unwrap();
			}
		});
		let client = tokio::task::spawn(async move {
			for i in 0..1024 {
				let message = format!("Hello #{}", i);
				let n_sent = client.send(message.as_bytes()).await.unwrap();
				assert_eq!(n_sent, message.len());
				let mut buf = vec![0u8; 1024];
				let n_received = client.recv(&mut buf).await.unwrap();
				assert_eq!(message.as_bytes(), &buf[..n_received]);
			}
		});
		tokio::join!(server, client)
	});
	client_result.expect("client failed");
	server_result.expect("server failed");
}
