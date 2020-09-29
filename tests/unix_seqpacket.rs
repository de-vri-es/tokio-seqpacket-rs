use tokio_seqpacket::UnixSeqpacket;
use assert2::{assert, let_assert};


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

	let mut runtime = tokio::runtime::Runtime::new().unwrap();
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
