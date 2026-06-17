use assert2::assert;
use tokio_seqpacket::UnixSeqpacket;

/// Test a simple send and recv call.
#[tokio::test]
async fn send_recv() {
	assert!(let Ok((a, b)) = UnixSeqpacket::pair());
	assert!(let Ok(12) = a.send(b"Hello world!").await);

	let mut buffer = [0u8; 128];
	assert!(let Ok(msg_info) = b.recv(&mut buffer).await);
	assert!(msg_info.bytes_read() == 12);
	assert!(!msg_info.truncated());
	assert!(&buffer[..12] == b"Hello world!");
}

/// Test that receiving a message partially sets the truncated flag.
#[tokio::test]
async fn recv_partial() {
	assert!(let Ok((a, b)) = UnixSeqpacket::pair());
	assert!(let Ok(12) = a.send(b"Hello world!").await);

	let mut buffer = [0u8; 5];
	assert!(let Ok(msg_info) = b.recv(&mut buffer).await);
	assert!(msg_info.bytes_read() == 5);
	assert!(msg_info.truncated());
	assert!(&buffer == b"Hello");
}

/// Test a simple send and peek call.
#[tokio::test]
async fn send_peek() {
	assert!(let Ok((a, b)) = UnixSeqpacket::pair());
	assert!(let Ok(12) = a.send(b"Hello world!").await);

	let mut buffer = [0u8; 128];

	// Peeking should not consume the message, so do it twice
	for _ in 0..2 {
		assert!(let Ok(msg_info) = b.peek(&mut buffer).await);
		assert!(msg_info.bytes_read() == 12);
		assert!(!msg_info.truncated());
		assert!(&buffer[..12] == b"Hello world!");
	}

	// We should still able to receive the message after peeking
	assert!(let Ok(msg_info) = b.recv(&mut buffer).await);
	assert!(msg_info.bytes_read() == 12);
	assert!(!msg_info.truncated());
	assert!(&buffer[..12] == b"Hello world!");
}

/// Test peeking a portion of a message with a small buffer.
#[tokio::test]
async fn peek_partial() {
	assert!(let Ok((a, b)) = UnixSeqpacket::pair());
	assert!(let Ok(12) = a.send(b"Hello world!").await);

	let mut buffer = [0u8; 128];
	assert!(let Ok(msg_info) = b.peek(&mut buffer[..5]).await);
	assert!(msg_info.bytes_read() == 5);
	assert!(msg_info.truncated());
	assert!(&buffer[..5] == b"Hello");

	// We should still able to receive the full message after peeking
	assert!(let Ok(msg_info) = b.recv(&mut buffer).await);
	assert!(msg_info.bytes_read() == 12);
	assert!(!msg_info.truncated());
	assert!(&buffer[..12] == b"Hello world!");
}

/// Record boundaries should be preserved
#[tokio::test]
async fn record_boundaries() {
	assert!(let Ok((a, b)) = UnixSeqpacket::pair());
	assert!(let Ok(12) = a.send(b"Hello world!").await);
	assert!(let Ok(12) = a.send(b"Byebye world").await);

	// Having sent two messages, we recv should return twice, with only a single message each
	// time.
	let mut buffer = [0u8; 128];
	assert!(let Ok(msg_info) = b.recv(&mut buffer).await);
	assert!(msg_info.bytes_read() == 12);
	assert!(&buffer[..12] == b"Hello world!");
	assert!(let Ok(msg_info) = b.recv(&mut buffer).await);
	assert!(msg_info.bytes_read() == 12);
	assert!(&buffer[..12] == b"Byebye world");
}

/// Test a send and receive call where the send wakes the recv task.
#[test]
fn send_recv_out_of_order() {
	use std::sync::atomic::{AtomicBool, Ordering};

	let runtime = tokio::runtime::Builder::new_current_thread()
		.enable_all()
		.build()
		.unwrap();
	let local = tokio::task::LocalSet::new();

	local.block_on(&runtime, async {
		// Atomic bools to verify things happen in the order we want.
		// We're using a local task set to ensure we're single threaded.
		static ABOUT_TO_READ: AtomicBool = AtomicBool::new(false);

		assert!(let Ok((a, b)) = UnixSeqpacket::pair());

		// Spawning a task shouldn't run anything until the current task awaits something.
		// Still, we use the atomic boolean to double-check that.
		let task = tokio::task::spawn_local(async move {
			assert!(ABOUT_TO_READ.load(Ordering::Relaxed) == true);
			assert!(let Ok(12) = a.send(b"Hello world!").await);
		});

		let mut buffer = [0u8; 128];
		ABOUT_TO_READ.store(true, Ordering::Relaxed);
		assert!(let Ok(msg_info) = b.recv(&mut buffer).await);
		assert!(msg_info.bytes_read() == 12);
		assert!(&buffer[..12] == b"Hello world!");

		assert!(let Ok(()) = task.await);
	});
}

/// Test a simple send_vectored and recv_vectored call.
#[tokio::test]
async fn send_recv_vectored() {
	use std::io::{IoSlice, IoSliceMut};

	assert!(let Ok((a, b)) = UnixSeqpacket::pair());
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
	assert!(let Ok(msg_info) = b.recv_vectored(&mut [
		IoSliceMut::new(&mut hello),
		IoSliceMut::new(&mut space),
		IoSliceMut::new(&mut world),
		IoSliceMut::new(&mut punct),
	]).await);
	assert!(msg_info.bytes_read() == 12);

	assert!(&hello == b"Hello");
	assert!(&space == b" ");
	assert!(&world == b"world");
	assert!(&punct == b"!");
}

/// Test a simple send_vectored and peek_vectored call.
#[tokio::test]
async fn send_peek_vectored() {
	use std::io::{IoSlice, IoSliceMut};

	assert!(let Ok((a, b)) = UnixSeqpacket::pair());
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

	// Peeking should not consume the message
	for _ in 0..2 {
		assert!(let Ok(msg_info) = b.peek_vectored(&mut [
			IoSliceMut::new(&mut hello),
			IoSliceMut::new(&mut space),
			IoSliceMut::new(&mut world),
			IoSliceMut::new(&mut punct),
		]).await);
		assert!(msg_info.bytes_read() == 12);

		assert!(&hello == b"Hello");
		assert!(&space == b" ");
		assert!(&world == b"world");
		assert!(&punct == b"!");
	}

	// We should still able to receive the message after peeking
	let mut buffer = [0u8; 12];
	assert!(let Ok(msg_info) = b.recv(&mut buffer).await);
	assert!(msg_info.bytes_read() == 12);
	assert!(&buffer[..12] == b"Hello world!");
}

#[test]
fn echo_loop() {
	let runtime = tokio::runtime::Builder::new_current_thread()
		.enable_all()
		.build()
		.unwrap();

	runtime.block_on(async {
		assert!(let Ok((client, server)) = UnixSeqpacket::pair());

		let server = tokio::task::spawn(async move {
			let mut buf = vec![0u8; 2048];
			loop {
				println!("waiting for next request");
				assert!(let Ok(msg_info) = server.recv(&mut buf).await);
				println!("received: {}", String::from_utf8_lossy(&buf[..msg_info.bytes_read()]));
				if msg_info.bytes_read() == 0 {
					break;
				}
				assert!(let Ok(_) = server.send(&buf[..msg_info.bytes_read()]).await);
			}
		});
		let client = tokio::task::spawn(async move {
			for i in 0..1024 {
				let message = format!("Hello #{}", i);
				assert!(let Ok(n_sent) = client.send(message.as_bytes()).await);
				assert!(n_sent == message.len());
				let mut buf = vec![0u8; 1024];
				assert!(let Ok(msg_info) = client.recv(&mut buf).await);
				assert!(message.as_bytes() == &buf[..msg_info.bytes_read()]);
			}
		});

		let (server_result, client_result) = tokio::join!(server, client);
		assert!(let Ok(()) = server_result);
		assert!(let Ok(()) = client_result);
	});
}

#[test]
fn multiple_waiters() {
	use std::sync::atomic::{AtomicUsize, Ordering};
	use std::sync::Arc;

	let runtime = tokio::runtime::Builder::new_current_thread()
		.enable_all()
		.build()
		.unwrap();

	runtime.block_on(async {
		assert!(let Ok((a, b)) = UnixSeqpacket::pair());
		let a = Arc::new(a);
		let b = Arc::new(b);
		let written = Arc::new(AtomicUsize::new(0));
		let received = Arc::new(AtomicUsize::new(0));

		let read1 = tokio::spawn({
			let a = a.clone();
			let written = written.clone();
			let received = received.clone();
			async move {
				let mut buffer = [0u8; 12];
				assert!(written.load(Ordering::Relaxed) == 0); // Double check that the test will cause recv() to park the current task.
				assert!(let Ok(msg_info) = a.recv(&mut buffer).await);
				assert!(msg_info.bytes_read() == 12);
				assert!(&buffer == b"Hello world!");
				received.fetch_add(1, Ordering::Relaxed);
			}
		});

		let read2 = tokio::spawn({
			let a = a.clone();
			let written = written.clone();
			let received = received.clone();
			async move {
				let mut buffer = [0u8; 12];
				assert!(written.load(Ordering::Relaxed) == 0); // Double check that the test will cause recv() to park the current task.
				assert!(let Ok(msg_info) = a.recv(&mut buffer).await);
				assert!(msg_info.bytes_read() == 12);
				assert!(&buffer == b"Hello world!");
				received.fetch_add(1, Ordering::Relaxed);
			}
		});

		let write = tokio::spawn(async move {
			// Give the readers some time to get parked.
			for _ in 0..10 {
				tokio::task::yield_now().await;
			}
			written.fetch_add(1, Ordering::Relaxed);
			assert!(let Ok(12) = b.send(b"Hello world!").await);
			written.fetch_add(1, Ordering::Relaxed);
			assert!(let Ok(12) = b.send(b"Hello world!").await);
		});

		let (read1, read2, write) = tokio::join!(read1, read2, write);
		assert!(let Ok(()) = read1);
		assert!(let Ok(()) = read2);
		assert!(let Ok(()) = write);
		assert!(received.load(Ordering::Relaxed) == 2);
	});
}
