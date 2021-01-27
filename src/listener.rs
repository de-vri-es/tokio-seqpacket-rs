use futures::future::poll_fn;
use std::os::unix::io::AsRawFd;
use std::os::unix::net::SocketAddr;
use std::path::Path;
use std::task::{Context, Poll};
use tokio::io::unix::AsyncFd;

use crate::UnixSeqpacket;

/// Listener for Unix seqpacket sockets.
pub struct UnixSeqpacketListener {
	io: AsyncFd<socket2::Socket>,
}

impl std::fmt::Debug for UnixSeqpacketListener {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		f.debug_struct("UnixSeqpacketListener")
			.field("fd", &self.io.get_ref().as_raw_fd())
			.finish()
	}
}

impl UnixSeqpacketListener {
	fn new(socket: socket2::Socket) -> std::io::Result<Self> {
		let io = AsyncFd::new(socket)?;
		Ok(Self { io })
	}

	/// Bind a new seqpacket listener to the given address.
	///
	/// The create listener will be ready to accept new connections.
	pub fn bind<P: AsRef<Path>>(address: P) -> std::io::Result<Self> {
		Self::bind_with_backlog(address, 128)
	}

	/// Bind a new seqpacket listener to the given address.
	///
	/// The create listener will be ready to accept new connections.
	///
	/// The `backlog` parameter is used to determine the size of connection queue.
	/// See `man 3 listen` for more information.
	pub fn bind_with_backlog<P: AsRef<Path>>(address: P, backlog: std::os::raw::c_int) -> std::io::Result<Self> {
		let address = socket2::SockAddr::unix(address)?;
		let socket = socket2::Socket::new(socket2::Domain::unix(), crate::socket_type(), None)?;
		socket.bind(&address)?;
		socket.listen(backlog)?;
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
		let (socket, addr) = loop {
			let mut ready_guard = ready!(self.io.poll_read_ready(cx)?);

			match ready_guard.try_io(|inner| inner.get_ref().accept()) {
				Ok(x) => break x?,
				Err(_would_block) => continue,
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
