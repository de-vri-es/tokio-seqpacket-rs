use ::mio::Ready;
use std::io::{IoSlice, IoSliceMut};
use std::os::unix::io::AsRawFd;
use std::path::Path;
use std::task::{Context, Poll};
use tokio::future::poll_fn;
use tokio::io::PollEvented;
use tokio::net::unix::UCred;

use crate::ancillary::SocketAncillary;

pub struct UnixSeqpacket {
	io: PollEvented<crate::mio::EventedSocket>,
}

impl std::fmt::Debug for UnixSeqpacket {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		f.debug_struct("UnixSeqpacket").field("fd", &self.io.get_ref().as_raw_fd()).finish()
	}
}

macro_rules! ready {
	($e:expr) => {
		match $e {
			Poll::Pending => return Poll::Pending,
			Poll::Ready(x) => x,
			}
	};
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
		#[allow(clippy::single_match)]
		match socket.connect(&address) {
			Err(e) => {
				if e.kind() != std::io::ErrorKind::WouldBlock {
					return Err(e);
				}
			},
			_ => (),
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

	/// Send data on the socket to the connected peer without blocking.
	pub fn poll_send(&mut self, cx: &mut Context, buffer: &[u8]) -> Poll<std::io::Result<usize>> {
		ready!(self.io.poll_write_ready(cx)?);

		match self.io.get_ref().send(buffer) {
			Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
				self.io.clear_write_ready(cx)?;
				Poll::Pending
			},
			x => Poll::Ready(x),
		}
	}

	/// Send data on the socket to the connected peer without blocking.
	pub fn poll_send_vectored(&mut self, cx: &mut Context, buffer: &[IoSlice]) -> Poll<std::io::Result<usize>> {
		self.poll_send_vectored_with_ancillary(cx, buffer, &mut SocketAncillary::new(&mut []))
	}

	/// Send data on the socket to the connected peer without blocking.
	pub fn poll_send_vectored_with_ancillary(
		&mut self,
		cx: &mut Context,
		buffer: &[IoSlice],
		ancillary: &mut SocketAncillary,
	) -> Poll<std::io::Result<usize>> {
		ready!(self.io.poll_write_ready(cx)?);

		match send_msg(self.io.get_ref(), buffer, ancillary) {
			Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
				self.io.clear_write_ready(cx)?;
				Poll::Pending
			},
			x => Poll::Ready(x),
		}
	}

	/// Send data on the socket to the connected peer.
	pub async fn send(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
		poll_fn(|cx| self.poll_send(cx, buffer)).await
	}

	/// Send data on the socket to the connected peer.
	pub async fn send_vectored(&mut self, buffer: &[IoSlice<'_>]) -> std::io::Result<usize> {
		poll_fn(|cx| self.poll_send_vectored(cx, buffer)).await
	}

	/// Send data on the socket to the connected peer.
	pub async fn send_with_ancillary(&mut self, buffer: &[IoSlice<'_>], ancillary: &mut SocketAncillary<'_>) -> std::io::Result<usize> {
		poll_fn(|cx| self.poll_send_vectored_with_ancillary(cx, buffer, ancillary)).await
	}

	/// Receive data on the socket from the connected peer without blocking.
	pub fn poll_recv(&mut self, cx: &mut Context, buffer: &mut [u8]) -> Poll<std::io::Result<usize>> {
		ready!(self.io.poll_read_ready(cx, Ready::readable())?);

		match self.io.get_ref().recv(buffer) {
			Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
				self.io.clear_read_ready(cx, Ready::readable())?;
				Poll::Pending
			},
			x => Poll::Ready(x),
		}
	}

	/// Receive data on the socket from the connected peer without blocking.
	pub fn poll_recv_vectored(&mut self, cx: &mut Context, buffer: &mut [IoSliceMut]) -> Poll<std::io::Result<usize>> {
		self.poll_recv_vectored_with_ancillary(cx, buffer, &mut SocketAncillary::new(&mut []))
	}

	/// Receive data on the socket from the connected peer without blocking.
	pub fn poll_recv_vectored_with_ancillary(
		&mut self,
		cx: &mut Context,
		buffer: &mut [IoSliceMut],
		ancillary: &mut SocketAncillary,
	) -> Poll<std::io::Result<usize>> {
		ready!(self.io.poll_read_ready(cx, Ready::readable())?);

		match recv_msg(self.io.get_ref(), buffer, ancillary) {
			Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
				self.io.clear_read_ready(cx, Ready::readable())?;
				Poll::Pending
			},
			x => Poll::Ready(x),
		}
	}

	/// Receive data on the socket from the connected peer.
	pub async fn recv(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
		poll_fn(|cx| self.poll_recv(cx, buffer)).await
	}

	/// Receive data on the socket from the connected peer.
	pub async fn recv_vectored(&mut self, buffer: &mut [IoSliceMut<'_>]) -> std::io::Result<usize> {
		poll_fn(|cx| self.poll_recv_vectored(cx, buffer)).await
	}

	/// Receive data on the socket from the connected peer.
	pub async fn recv_vectored_with_ancillary(
		&mut self,
		buffer: &mut [IoSliceMut<'_>],
		ancillary: &mut SocketAncillary<'_>,
	) -> std::io::Result<usize> {
		poll_fn(|cx| self.poll_recv_vectored_with_ancillary(cx, buffer, ancillary)).await
	}

	/// Shuts down the read, write, or both halves of this connection.
	///
	/// This function will cause all pending and future I/O calls on the
	/// specified portions to immediately return with an appropriate value
	/// (see the documentation of `Shutdown`).
	pub fn shutdown(&self, how: std::net::Shutdown) -> std::io::Result<()> {
		self.io.get_ref().shutdown(how)
	}
}

const SEND_MSG_DEFAULT_FLAGS: std::os::raw::c_int = libc::MSG_NOSIGNAL;
const RECV_MSG_DEFAULT_FLAGS: std::os::raw::c_int = libc::MSG_NOSIGNAL | libc::MSG_CMSG_CLOEXEC;

#[cfg(any(target_os = "android", all(target_os = "linux", target_env = "gnu")))]
type CmgLen = usize;

#[cfg(any(
	target_os = "dragonfly",
	target_os = "emscripten",
	target_os = "freebsd",
	all(target_os = "linux", target_env = "musl",),
	target_os = "netbsd",
	target_os = "openbsd",
))]
type CmgLen = std::os::raw::c_int;

fn send_msg(socket: &socket2::Socket, buffer: &[IoSlice], ancillary: &mut SocketAncillary) -> std::io::Result<usize> {
	ancillary.truncated = false;

	let control_data = match ancillary.len() {
		0 => std::ptr::null_mut(),
		_ => ancillary.buffer.as_mut_ptr() as *mut std::os::raw::c_void,
	};

	let fd = socket.as_raw_fd();
	let header = libc::msghdr {
		msg_name: std::ptr::null_mut(),
		msg_namelen: 0,
		msg_iov: buffer.as_ptr() as *mut libc::iovec,
		msg_iovlen: buffer.len(),
		msg_flags: 0,
		msg_control: control_data,
		msg_controllen: ancillary.len() as CmgLen,
	};

	unsafe { check_returned_size(libc::sendmsg(fd, &header as *const _, SEND_MSG_DEFAULT_FLAGS)) }
}

fn recv_msg(socket: &socket2::Socket, buffer: &mut [IoSliceMut], ancillary: &mut SocketAncillary) -> std::io::Result<usize> {
	let control_data = match ancillary.len() {
		0 => std::ptr::null_mut(),
		_ => ancillary.buffer.as_mut_ptr() as *mut std::os::raw::c_void,
	};

	let fd = socket.as_raw_fd();
	let mut header = libc::msghdr {
		msg_name: std::ptr::null_mut(),
		msg_namelen: 0,
		msg_iov: buffer.as_ptr() as *mut libc::iovec,
		msg_iovlen: buffer.len(),
		msg_flags: 0,
		msg_control: control_data,
		msg_controllen: ancillary.capacity() as CmgLen,
	};
	let size = unsafe { check_returned_size(libc::recvmsg(fd, &mut header as *mut _, RECV_MSG_DEFAULT_FLAGS))? };
	ancillary.truncated = header.msg_flags & libc::MSG_CTRUNC != 0;
	Ok(size)
}

fn check_returned_size(ret: isize) -> std::io::Result<usize> {
	if ret < 0 {
		Err(std::io::Error::last_os_error())
	} else {
		Ok(ret as usize)
	}
}
