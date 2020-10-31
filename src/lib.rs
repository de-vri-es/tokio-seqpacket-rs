//! Unix seqpacket sockets for [tokio](https://docs.rs/tokio).
//!
//! Seqpacket sockets combine a number of useful properties:
//! * They are connection oriented.
//! * They guarantee in-order message delivery.
//! * They provide datagrams with well-defined semantics for passing along file descriptors.
//!
//! These properties make seqpacket sockets very well suited for local servers that need to pass file-descriptors around with their clients.
//!
//! You can create a [`UnixSeqpacketListener`] to start accepting connections,
//! or create a [`UnixSeqpacket`] to connect to a listening socket.
//! You can also create a pair of connected sockets with [`UnixSeqpacket::pair()`].
//!
//! # Passing file descriptors and other ancillary data.
//!
//! You can use [`send_vectored_with_ancillary`](UnixSeqpacket::send_vectored_with_ancillary) and [`recv_vectored_with_ancillary`](UnixSeqpacket::recv_vectored_with_ancillary)
//! to send and receive ancillary data.
//! This can be used to pass file descriptors and unix credentials over sockets.
//!
//! # `&self` versus `&mut self`
//!
//! Seqpacket sockets have well-defined semantics when sending or receiving on the same socket from different threads.
//! Although the order is not guaranteed in that scenario, each datagram will be delivered intact.
//! As such, you might except the socket to allow sending and receiving from a shared reference.
//!
//! However, the tokio runtime only supports a single read task and a single write task waiting for readiness events.
//! For that reason, you can only send and receive messages through an exclusive reference.
//!
//! You can split the socket into a read and write half using [`UnixSeqpacket::split()`].
//!
//! # Example
//! ```no_run
//! # async fn foo() -> Result<(), Box<dyn std::error::Error>> {
//! use tokio_seqpacket::UnixSeqpacket;
//!
//! let mut socket = UnixSeqpacket::connect("/run/foo.sock").await?;
//! socket.send(b"Hello!").await?;
//!
//! let mut buffer = [0u8; 128];
//! let len = socket.recv(&mut buffer).await?;
//! println!("{}", String::from_utf8_lossy(&buffer[..len]));
//! # Ok(())
//! # }
//! ```

#![warn(missing_docs)]

macro_rules! ready {
	($e:expr) => {
		match $e {
			Poll::Pending => return Poll::Pending,
			Poll::Ready(x) => x,
		}
	};
}

pub mod ancillary;
mod listener;
mod socket;
mod split;
mod ucred;

pub use listener::UnixSeqpacketListener;
pub use socket::UnixSeqpacket;
pub use split::{ReadHalf, WriteHalf};
pub use ucred::UCred;

/// Get the socket type for a close-on-exec non-blocking seqpacket socket.
fn socket_type() -> socket2::Type {
	socket2::Type::seqpacket().cloexec().non_blocking()
}

/// Convert a [`socket2::SockAddr`] to a [`std::os::unix::net::SocketAddr`].
fn sockaddr_as_unix(addr: &socket2::SockAddr) -> Option<std::os::unix::net::SocketAddr> {
	if addr.family() != libc::AF_LOCAL as libc::sa_family_t {
		return None;
	}

	#[allow(dead_code)]
	struct SocketAddrInternal {
		addr: libc::sockaddr_un,
		len: libc::socklen_t,
	}

	unsafe {
		let internal = SocketAddrInternal {
			addr: std::ptr::read(addr.as_ptr() as *const libc::sockaddr_un),
			len: addr.len(),
		};
		Some(std::mem::transmute(internal))
	}
}
