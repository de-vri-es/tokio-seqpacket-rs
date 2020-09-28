use std::path::Path;
use tokio::future::poll_fn;
use tokio::io::PollEvented;
use std::os::unix::net::SocketAddr;
use std::task::{Context, Poll};
use ::mio::Ready;

use crate::UnixSeqpacket;

pub struct UnixSeqpacketListener {
	io: PollEvented<crate::mio::EventedSocket>,
}

impl UnixSeqpacketListener {
	fn new(socket: socket2::Socket) -> std::io::Result<Self> {
		let socket = crate::mio::EventedSocket::new(socket);
		let io = PollEvented::new(socket)?;
		Ok(Self { io })
	}

	/// Bind a new seqpacket listener to the given address.
	///
	/// The create listener will be ready to accept new connections.
	pub fn bind<P: AsRef<Path>>(address: P) -> std::io::Result<Self> {
		let address = socket2::SockAddr::unix(address)?;
		let socket = socket2::Socket::new(socket2::Domain::unix(), crate::socket_type(), None)?;
		socket.bind(&address)?;
		socket.listen(128)?;
		Self::new(socket)
	}

	/// Get the socket address of the local half of this connection.
	pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
		let addr = self.io.get_ref().local_addr()?;
		Ok(crate::sockaddr_as_unix(&addr).unwrap())
	}

	/// Get the value of the `SO_ERROR` option.
	pub fn take_error(&self) -> std::io::Result<Option<std::io::Error>> {
		self.io.get_ref().take_error()
	}

	/// Check if there is a connection ready to accept.
	pub fn poll_accept(&mut self, cx: &mut Context) -> Poll<std::io::Result<(UnixSeqpacket, SocketAddr)>> {
		let ready = self.io.poll_read_ready(cx, Ready::readable())?;
		if ready.is_pending() {
			return Poll::Pending;
		}

		let (socket, addr) = match self.io.get_ref().accept() {
			Ok(x) => x,
			Err(e) => if e.kind() == std::io::ErrorKind::WouldBlock {
				self.io.clear_read_ready(cx, Ready::readable())?;
				return Poll::Pending;
			} else {
				return Poll::Ready(Err(e));
			}
		};

		socket.set_nonblocking(true)?;
		let addr = crate::sockaddr_as_unix(&addr).unwrap();
		Poll::Ready(Ok((UnixSeqpacket::new(socket)?, addr)))
	}

	/// Accept a new incoming connection on the listener.
	pub async fn accept(&mut self) -> std::io::Result<(UnixSeqpacket, SocketAddr)> {
		poll_fn(|cx| self.poll_accept(cx)).await
	}
}
