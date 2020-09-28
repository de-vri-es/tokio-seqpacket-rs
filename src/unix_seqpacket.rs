use std::path::Path;
use tokio::future::poll_fn;
use tokio::io::PollEvented;
use tokio::net::unix::UCred;
use std::task::{Context, Poll};
use ::mio::Ready;

pub struct UnixSeqpacket {
	io: PollEvented<crate::mio::EventedSocket>,
}

impl UnixSeqpacket {
	pub(crate) fn new(socket: socket2::Socket) -> std::io::Result<Self> {
		let socket = crate::mio::EventedSocket::new(socket);
		let io = PollEvented::new(socket)?;
		Ok(Self { io })
	}

	/// Connect a new seqpacket socket to the given address.
	pub async fn connect<P: AsRef<Path>>(address: P) -> std::io::Result<Self> {
		let address = socket2::SockAddr::unix(address)?;
		let socket = socket2::Socket::new(socket2::Domain::unix(), crate::socket_type(), None)?;
		match socket.connect(&address) {
			Ok(()) => (),
			Err(e) => if e.kind() != std::io::ErrorKind::WouldBlock {
				return Err(e);
			}
		};

		let socket = Self::new(socket)?;
		poll_fn(|cx| socket.io.poll_write_ready(cx)).await?;
		Ok(socket)
	}

	/// Create a pair of connected seqpacket sockets.
	pub fn pair() -> std::io::Result<(Self, Self)> {
		let (a, b) = socket2::Socket::pair(socket2::Domain::unix(), crate::socket_type(), None)?;
		let a = Self::new(a)?;
		let b = Self::new(b)?;
		Ok((a, b))
	}

	/// Get the socket address of the local half of this connection.
	pub fn local_addr(&self) -> std::io::Result<std::os::unix::net::SocketAddr> {
		let addr = self.io.get_ref().local_addr()?;
		Ok(crate::sockaddr_as_unix(&addr).unwrap())
	}

	/// Get the socket address of the remote half of this connection.
	pub fn peer_addr(&self) -> std::io::Result<std::os::unix::net::SocketAddr> {
		let addr = self.io.get_ref().peer_addr()?;
		Ok(crate::sockaddr_as_unix(&addr).unwrap())
	}

	/// Get the effective credentials of the process which called `connect` or `pair`.
	pub fn peer_cred(&self) -> std::io::Result<UCred> {
		crate::ucred::get_peer_cred(self.io.get_ref())
	}

	/// Get the value of the `SO_ERROR` option.
	pub fn take_error(&self) -> std::io::Result<Option<std::io::Error>> {
		self.io.get_ref().take_error()
	}

	/// Send data on the socket to the connected peer.
	pub async fn send(&mut self, buf: &[u8]) -> std::io::Result<usize> {
		poll_fn(|cx| self.poll_send_priv(cx, buf)).await
	}

	/// Receove data on the socket from the connected peer.
	pub async fn recv(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
		poll_fn(|cx| self.poll_recv_priv(cx, buf)).await
	}

	/// Shuts down the read, write, or both halves of this connection.
	///
	/// This function will cause all pending and future I/O calls on the
	/// specified portions to immediately return with an appropriate value
	/// (see the documentation of `Shutdown`).
	pub fn shutdown(&self, how: std::net::Shutdown) -> std::io::Result<()> {
		self.io.get_ref().shutdown(how)
	}

	pub fn poll_send_priv(&self, cx: &mut Context, buf: &[u8]) -> Poll<std::io::Result<usize>> {
		let ready = self.io.poll_write_ready(cx)?;
		if ready.is_pending() {
			return Poll::Pending;
		}

		match self.io.get_ref().send(buf) {
			Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
				self.io.clear_write_ready(cx)?;
				Poll::Pending
			}
			x => Poll::Ready(x),
		}
	}

	pub fn poll_recv_priv(&self, cx: &mut Context, buf: &mut [u8]) -> Poll<std::io::Result<usize>> {
		let ready = self.io.poll_read_ready(cx, Ready::readable())?;
		if ready.is_pending() {
			return Poll::Pending;
		}

		match self.io.get_ref().recv(buf) {
			Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
				self.io.clear_read_ready(cx, Ready::readable())?;
				Poll::Pending
			}
			x => Poll::Ready(x),
		}
	}
}
