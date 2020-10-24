use std::io::{IoSlice, IoSliceMut};
use std::os::unix::io::AsRawFd;
use std::path::Path;
use std::task::{Context, Poll};
use futures::future::poll_fn;
use tokio::io::unix::AsyncFd;
use tokio::net::unix::UCred;

use crate::ancillary::SocketAncillary;

/// Unix seqpacket socket.
pub struct UnixSeqpacket {
	io: AsyncFd<socket2::Socket>,
}

impl std::fmt::Debug for UnixSeqpacket {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		f.debug_struct("UnixSeqpacket").field("fd", &self.io.get_ref().as_raw_fd()).finish()
	}
}

impl UnixSeqpacket {
	pub(crate) fn new(socket: socket2::Socket) -> std::io::Result<Self> {
		let io = AsyncFd::new(socket)?;
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
		socket.io.writable().await?.retain_ready();
		Ok(socket)
	}

	/// Create a pair of connected seqpacket sockets.
	pub fn pair() -> std::io::Result<(Self, Self)> {
		let (a, b) = socket2::Socket::pair(socket2::Domain::unix(), crate::socket_type(), None)?;
		let a = Self::new(a)?;
		let b = Self::new(b)?;
		Ok((a, b))
	}

	/// Split the socket in a read half and a write half.
	///
	/// The two halves borrow `self`, so they can not be moved into different tasks.
	/// An owned version of `Self::split()` is still planned.
	pub fn split(&mut self) -> (crate::ReadHalf, crate::WriteHalf) {
		unsafe {
			let read_half = crate::ReadHalf::new(self);
			let write_half = crate::WriteHalf::new(self);
			(read_half, write_half)
		}
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

	/// Try to send data on the socket to the connected peer without blocking.
	///
	/// If the socket is not ready yet, the current task is scheduled to wake up when the socket becomes writeable.
	pub fn poll_send(&mut self, cx: &mut Context, buffer: &[u8]) -> Poll<std::io::Result<usize>> {
		poll_send(self, cx, buffer)
	}

	/// Try to send data on the socket to the connected peer without blocking.
	///
	/// If the socket is not ready yet, the current task is scheduled to wake up when the socket becomes writeable.
	pub fn poll_send_vectored(&mut self, cx: &mut Context, buffer: &[IoSlice]) -> Poll<std::io::Result<usize>> {
		poll_send_vectored(self, cx, buffer)
	}

	/// Try to send data with ancillary data on the socket to the connected peer without blocking.
	///
	/// If the socket is not ready yet, the current task is scheduled to wake up when the socket becomes writeable.
	pub fn poll_send_vectored_with_ancillary(
		&mut self,
		cx: &mut Context,
		buffer: &[IoSlice],
		ancillary: &mut SocketAncillary,
	) -> Poll<std::io::Result<usize>> {
		poll_send_vectored_with_ancillary(self, cx, buffer, ancillary)
	}

	/// Send data on the socket to the connected peer.
	pub async fn send(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
		poll_fn(|cx| self.poll_send(cx, buffer)).await
	}

	/// Send data on the socket to the connected peer.
	pub async fn send_vectored(&mut self, buffer: &[IoSlice<'_>]) -> std::io::Result<usize> {
		poll_fn(|cx| self.poll_send_vectored(cx, buffer)).await
	}

	/// Send data with ancillary data on the socket to the connected peer.
	pub async fn send_vectored_with_ancillary(
		&mut self,
		buffer: &[IoSlice<'_>],
		ancillary: &mut SocketAncillary<'_>,
	) -> std::io::Result<usize> {
		poll_fn(|cx| self.poll_send_vectored_with_ancillary(cx, buffer, ancillary)).await
	}

	/// Try to receive data on the socket from the connected peer without blocking.
	///
	/// If there is no data ready yet, the current task is scheduled to wake up when the socket becomes readable.
	pub fn poll_recv(&mut self, cx: &mut Context, buffer: &mut [u8]) -> Poll<std::io::Result<usize>> {
		poll_recv(self, cx, buffer)
	}

	/// Try to receive data on the socket from the connected peer without blocking.
	///
	/// If there is no data ready yet, the current task is scheduled to wake up when the socket becomes readable.
	pub fn poll_recv_vectored(&mut self, cx: &mut Context, buffer: &mut [IoSliceMut]) -> Poll<std::io::Result<usize>> {
		poll_recv_vectored(self, cx, buffer)
	}

	/// Try to receive data with ancillary data on the socket from the connected peer without blocking.
	///
	/// If there is no data ready yet, the current task is scheduled to wake up when the socket becomes readable.
	pub fn poll_recv_vectored_with_ancillary(
		&mut self,
		cx: &mut Context,
		buffer: &mut [IoSliceMut],
		ancillary: &mut SocketAncillary,
	) -> Poll<std::io::Result<usize>> {
		poll_recv_vectored_with_ancillary(self, cx, buffer, ancillary)
	}

	/// Receive data on the socket from the connected peer.
	pub async fn recv(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
		poll_fn(|cx| self.poll_recv(cx, buffer)).await
	}

	/// Receive data on the socket from the connected peer.
	pub async fn recv_vectored(&mut self, buffer: &mut [IoSliceMut<'_>]) -> std::io::Result<usize> {
		poll_fn(|cx| self.poll_recv_vectored(cx, buffer)).await
	}

	/// Receive data with ancillary data on the socket from the connected peer.
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
type CmsgLen = usize;

#[cfg(any(
	target_os = "dragonfly",
	target_os = "emscripten",
	target_os = "freebsd",
	all(target_os = "linux", target_env = "musl",),
	target_os = "netbsd",
	target_os = "openbsd",
))]
type CmsgLen = std::os::raw::c_int;

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
		msg_controllen: ancillary.len() as CmsgLen,
	};

	unsafe { check_returned_size(libc::sendmsg(fd, &header as *const _, SEND_MSG_DEFAULT_FLAGS)) }
}

fn recv_msg(socket: &socket2::Socket, buffer: &mut [IoSliceMut], ancillary: &mut SocketAncillary) -> std::io::Result<usize> {
	let control_data = match ancillary.capacity() {
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
		msg_controllen: ancillary.capacity() as CmsgLen,
	};
	let size = unsafe { check_returned_size(libc::recvmsg(fd, &mut header as *mut _, RECV_MSG_DEFAULT_FLAGS))? };
	ancillary.truncated = header.msg_flags & libc::MSG_CTRUNC != 0;
	ancillary.length = header.msg_controllen as usize;
	Ok(size)
}

fn check_returned_size(ret: isize) -> std::io::Result<usize> {
	if ret < 0 {
		Err(std::io::Error::last_os_error())
	} else {
		Ok(ret as usize)
	}
}

/// Send data on the socket to the connected peer without blocking.
pub(crate) fn poll_send(socket: &UnixSeqpacket, cx: &mut Context, buffer: &[u8]) -> Poll<std::io::Result<usize>> {
	let mut ready_guard = ready!(socket.io.poll_write_ready(cx)?);

	match socket.io.get_ref().send(buffer) {
		Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
			ready_guard.clear_ready();
			Poll::Pending
		},
		x => Poll::Ready(x),
	}
}

/// Send data on the socket to the connected peer without blocking.
pub(crate) fn poll_send_vectored(socket: &UnixSeqpacket, cx: &mut Context, buffer: &[IoSlice]) -> Poll<std::io::Result<usize>> {
	poll_send_vectored_with_ancillary(socket, cx, buffer, &mut SocketAncillary::new(&mut []))
}

/// Send data on the socket to the connected peer without blocking.
pub(crate) fn poll_send_vectored_with_ancillary(
	socket: &UnixSeqpacket,
	cx: &mut Context,
	buffer: &[IoSlice],
	ancillary: &mut SocketAncillary,
) -> Poll<std::io::Result<usize>> {
	let mut ready_guard = ready!(socket.io.poll_write_ready(cx)?);

	match send_msg(socket.io.get_ref(), buffer, ancillary) {
		Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
			ready_guard.clear_ready();
			Poll::Pending
		},
		x => Poll::Ready(x),
	}
}

/// Receive data on the socket from the connected peer without blocking.
pub(crate) fn poll_recv(socket: &UnixSeqpacket, cx: &mut Context, buffer: &mut [u8]) -> Poll<std::io::Result<usize>> {
	let mut ready_guard = ready!(socket.io.poll_read_ready(cx)?);

	match socket.io.get_ref().recv(buffer) {
		Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
			ready_guard.clear_ready();
			Poll::Pending
		},
		x => Poll::Ready(x),
	}
}

/// Receive data on the socket from the connected peer without blocking.
pub(crate) fn poll_recv_vectored(socket: &UnixSeqpacket, cx: &mut Context, buffer: &mut [IoSliceMut]) -> Poll<std::io::Result<usize>> {
	poll_recv_vectored_with_ancillary(socket, cx, buffer, &mut SocketAncillary::new(&mut []))
}

/// Receive data on the socket from the connected peer without blocking.
pub(crate) fn poll_recv_vectored_with_ancillary(
	socket: &UnixSeqpacket,
	cx: &mut Context,
	buffer: &mut [IoSliceMut],
	ancillary: &mut SocketAncillary,
) -> Poll<std::io::Result<usize>> {
	let mut ready_guard = ready!(socket.io.poll_read_ready(cx)?);

	match recv_msg(socket.io.get_ref(), buffer, ancillary) {
		Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
			ready_guard.clear_ready();
			Poll::Pending
		},
		x => Poll::Ready(x),
	}
}
