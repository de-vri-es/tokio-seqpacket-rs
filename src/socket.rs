use std::convert::TryInto;
use std::io::{IoSlice, IoSliceMut};
use std::os::unix::io::{AsRawFd, IntoRawFd};
use std::path::Path;
use std::task::{Context, Poll};
use tokio::io::unix::AsyncFd;

use crate::ancillary::SocketAncillary;
use crate::UCred;

/// Unix seqpacket socket.
pub struct UnixSeqpacket {
	io: AsyncFd<socket2::Socket>,
}

impl std::fmt::Debug for UnixSeqpacket {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		f.debug_struct("UnixSeqpacket")
			.field("fd", &self.io.get_ref().as_raw_fd())
			.finish()
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

	/// Wrap a raw file descriptor as [`UnixSeqpacket`].
	///
	/// Registration of the file descriptor with the tokio runtime may fail.
	/// For that reason, this function returns a [`std::io::Result`].
	///
	/// # Safety
	/// This function is unsafe because the socket assumes it is the sole owner of the file descriptor.
	/// Usage of this function could accidentally allow violating this contract
	/// which can cause memory unsafety in code that relies on it being true.
	pub unsafe fn from_raw_fd(fd: std::os::unix::io::RawFd) -> std::io::Result<Self> {
		use std::os::unix::io::FromRawFd;
		Self::new(socket2::Socket::from_raw_fd(fd))
	}

	/// Get the raw file descriptor of the socket.
	pub fn as_raw_fd(&self) -> std::os::unix::io::RawFd {
		self.io.as_raw_fd()
	}

	/// Deregister the socket from the tokio runtime and return the inner file descriptor.
	pub fn into_raw_fd(self) -> std::os::unix::io::RawFd {
		self.io.into_inner().into_raw_fd()
	}

	#[doc(hidden)]
	#[deprecated(
		since = "0.4.0",
		note = "all I/O functions now take a shared reference to self, so splitting is no longer necessary"
	)]
	pub fn split(&self) -> (&Self, &Self) {
		(self, self)
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
	///
	/// Note that this is not necessarily the process that currently has the file descriptor
	/// of the other side of the connection.
	pub fn peer_cred(&self) -> std::io::Result<UCred> {
		UCred::from_socket_peer(&self.io)
	}

	/// Get the value of the `SO_ERROR` option.
	pub fn take_error(&self) -> std::io::Result<Option<std::io::Error>> {
		self.io.get_ref().take_error()
	}

	/// Try to send data on the socket to the connected peer without blocking.
	///
	/// If the socket is not ready yet, the current task is scheduled to wake up when the socket becomes writeable.
	///
	/// Note that unlike [`Self::send`], only the last task calling this function will be woken up.
	/// For that reason, it is preferable to use the async functions rather than polling functions when possible.
	pub fn poll_send(&self, cx: &mut Context, buffer: &[u8]) -> Poll<std::io::Result<usize>> {
		loop {
			let mut ready_guard = ready!(self.io.poll_write_ready(cx)?);

			match ready_guard.try_io(|inner| inner.get_ref().send(buffer)) {
				Ok(result) => return Poll::Ready(result),
				Err(_would_block) => continue,
			}
		}
	}

	/// Try to send data on the socket to the connected peer without blocking.
	///
	/// If the socket is not ready yet, the current task is scheduled to wake up when the socket becomes writeable.
	///
	/// Note that unlike [`Self::send_vectored`], only the last task calling this function will be woken up.
	/// For that reason, it is preferable to use the async functions rather than polling functions when possible.
	pub fn poll_send_vectored(&self, cx: &mut Context, buffer: &[IoSlice]) -> Poll<std::io::Result<usize>> {
		self.poll_send_vectored_with_ancillary(cx, buffer, &mut SocketAncillary::new(&mut []))
	}

	/// Try to send data with ancillary data on the socket to the connected peer without blocking.
	///
	/// If the socket is not ready yet, the current task is scheduled to wake up when the socket becomes writeable.
	///
	/// Note that unlike [`Self::send_vectored_with_ancillary`], only the last task calling this function will be woken up.
	/// For that reason, it is preferable to use the async functions rather than polling functions when possible.
	pub fn poll_send_vectored_with_ancillary(
		&self,
		cx: &mut Context,
		buffer: &[IoSlice],
		ancillary: &mut SocketAncillary,
	) -> Poll<std::io::Result<usize>> {
		loop {
			let mut ready_guard = ready!(self.io.poll_write_ready(cx)?);
			match ready_guard.try_io(|inner| send_msg(inner.get_ref(), buffer, ancillary)) {
				Ok(result) => return Poll::Ready(result),
				Err(_would_block) => continue,
			}
		}
	}

	/// Send data on the socket to the connected peer.
	///
	/// This function is safe to call concurrently from different tasks.
	/// Although no order is guaranteed, all calling tasks will try to complete the asynchronous action.
	pub async fn send(&self, buffer: &[u8]) -> std::io::Result<usize> {
		loop {
			let mut ready_guard = self.io.writable().await?;

			match ready_guard.try_io(|inner| inner.get_ref().send(buffer)) {
				Ok(result) => return result,
				Err(_would_block) => continue,
			}
		}
	}

	/// Send data on the socket to the connected peer.
	///
	/// This function is safe to call concurrently from different tasks.
	/// Although no order is guaranteed, all calling tasks will try to complete the asynchronous action.
	pub async fn send_vectored(&self, buffer: &[IoSlice<'_>]) -> std::io::Result<usize> {
		self.send_vectored_with_ancillary(buffer, &mut SocketAncillary::new(&mut [])).await
	}

	/// Send data with ancillary data on the socket to the connected peer.
	///
	/// This function is safe to call concurrently from different tasks.
	/// Although no order is guaranteed, all calling tasks will try to complete the asynchronous action.
	pub async fn send_vectored_with_ancillary(
		&self,
		buffer: &[IoSlice<'_>],
		ancillary: &mut SocketAncillary<'_>,
	) -> std::io::Result<usize> {
		loop {
			let mut ready_guard = self.io.writable().await?;
			match ready_guard.try_io(|inner| send_msg(inner.get_ref(), buffer, ancillary)) {
				Ok(result) => return result,
				Err(_would_block) => continue,
			}
		}
	}

	/// Try to receive data on the socket from the connected peer without blocking.
	///
	/// If there is no data ready yet, the current task is scheduled to wake up when the socket becomes readable.
	///
	/// Note that unlike [`Self::recv`], only the last task calling this function will be woken up.
	/// For that reason, it is preferable to use the async functions rather than polling functions when possible.
	pub fn poll_recv(&self, cx: &mut Context, buffer: &mut [u8]) -> Poll<std::io::Result<usize>> {
		loop {
			let mut ready_guard = ready!(self.io.poll_read_ready(cx)?);
			match ready_guard.try_io(|inner| inner.get_ref().recv(buffer)) {
				Ok(result) => return Poll::Ready(result),
				Err(_would_block) => continue,
			}
		}
	}

	/// Try to receive data on the socket from the connected peer without blocking.
	///
	/// If there is no data ready yet, the current task is scheduled to wake up when the socket becomes readable.
	///
	/// Note that unlike [`Self::recv_vectored`], only the last task calling this function will be woken up.
	/// For that reason, it is preferable to use the async functions rather than polling functions when possible.
	pub fn poll_recv_vectored(&self, cx: &mut Context, buffer: &mut [IoSliceMut]) -> Poll<std::io::Result<usize>> {
		self.poll_recv_vectored_with_ancillary(cx, buffer, &mut SocketAncillary::new(&mut []))
	}

	/// Try to receive data with ancillary data on the socket from the connected peer without blocking.
	///
	/// If there is no data ready yet, the current task is scheduled to wake up when the socket becomes readable.
	///
	/// Note that unlike [`Self::recv_vectored_with_ancillary`], only the last task calling this function will be woken up.
	/// For that reason, it is preferable to use the async functions rather than polling functions when possible.
	pub fn poll_recv_vectored_with_ancillary(
		&self,
		cx: &mut Context,
		buffer: &mut [IoSliceMut],
		ancillary: &mut SocketAncillary,
	) -> Poll<std::io::Result<usize>> {
		loop {
			let mut ready_guard = ready!(self.io.poll_read_ready(cx)?);

			match ready_guard.try_io(|inner| recv_msg(inner.get_ref(), buffer, ancillary)) {
				Ok(result) => return Poll::Ready(result),
				Err(_would_block) => continue,
			}
		}
	}

	/// Receive data on the socket from the connected peer.
	///
	/// This function is safe to call concurrently from different tasks.
	/// Although no order is guaranteed, all calling tasks will try to complete the asynchronous action.
	pub async fn recv(&self, buffer: &mut [u8]) -> std::io::Result<usize> {
		loop {
			let mut ready_guard = self.io.readable().await?;

			match ready_guard.try_io(|inner| inner.get_ref().recv(buffer)) {
				Ok(result) => return result,
				Err(_would_block) => continue,
			}
		}
	}

	/// Receive data on the socket from the connected peer.
	///
	/// This function is safe to call concurrently from different tasks.
	/// Although no order is guaranteed, all calling tasks will try to complete the asynchronous action.
	pub async fn recv_vectored(&self, buffer: &mut [IoSliceMut<'_>]) -> std::io::Result<usize> {
		self.recv_vectored_with_ancillary(buffer, &mut SocketAncillary::new(&mut [])).await
	}

	/// Receive data with ancillary data on the socket from the connected peer.
	///
	/// This function is safe to call concurrently from different tasks.
	/// Although no order is guaranteed, all calling tasks will try to complete the asynchronous action.
	pub async fn recv_vectored_with_ancillary(
		&self,
		buffer: &mut [IoSliceMut<'_>],
		ancillary: &mut SocketAncillary<'_>,
	) -> std::io::Result<usize> {
		loop {
			let mut ready_guard = self.io.readable().await?;

			match ready_guard.try_io(|inner| recv_msg(inner.get_ref(), buffer, ancillary)) {
				Ok(result) => return result,
				Err(_would_block) => continue,
			}
		}
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

impl AsRawFd for UnixSeqpacket {
	fn as_raw_fd(&self) -> std::os::unix::io::RawFd {
		self.as_raw_fd()
	}
}

impl IntoRawFd for UnixSeqpacket {
	fn into_raw_fd(self) -> std::os::unix::io::RawFd {
		self.into_raw_fd()
	}
}

const SEND_MSG_DEFAULT_FLAGS: std::os::raw::c_int = libc::MSG_NOSIGNAL;
const RECV_MSG_DEFAULT_FLAGS: std::os::raw::c_int = libc::MSG_NOSIGNAL | libc::MSG_CMSG_CLOEXEC;

fn send_msg(socket: &socket2::Socket, buffer: &[IoSlice], ancillary: &mut SocketAncillary) -> std::io::Result<usize> {
	ancillary.truncated = false;

	let control_data = match ancillary.len() {
		0 => std::ptr::null_mut(),
		_ => ancillary.buffer.as_mut_ptr() as *mut std::os::raw::c_void,
	};

	let fd = socket.as_raw_fd();
	let mut header: libc::msghdr = unsafe { std::mem::zeroed() };
	header.msg_name = std::ptr::null_mut();
	header.msg_namelen = 0;
	header.msg_iov = buffer.as_ptr() as *mut libc::iovec;
	// This is not a no-op on all platforms.
	#[allow(clippy::useless_conversion)]
	{
		header.msg_iovlen = buffer.len().try_into()
			.map_err(|_| std::io::ErrorKind::InvalidInput)?;
	}
	header.msg_flags = 0;
	header.msg_control = control_data;
	// This is not a no-op on all platforms.
	#[allow(clippy::useless_conversion)]
	{
		header.msg_controllen = ancillary.len().try_into()
			.map_err(|_| std::io::ErrorKind::InvalidInput)?;
	}

	unsafe { check_returned_size(libc::sendmsg(fd, &header as *const _, SEND_MSG_DEFAULT_FLAGS)) }
}

fn recv_msg(
	socket: &socket2::Socket,
	buffer: &mut [IoSliceMut],
	ancillary: &mut SocketAncillary,
) -> std::io::Result<usize> {
	let control_data = match ancillary.capacity() {
		0 => std::ptr::null_mut(),
		_ => ancillary.buffer.as_mut_ptr() as *mut std::os::raw::c_void,
	};

	let fd = socket.as_raw_fd();
	let mut header: libc::msghdr = unsafe { std::mem::zeroed() };
	header.msg_name = std::ptr::null_mut();
	header.msg_namelen = 0;
	header.msg_iov = buffer.as_ptr() as *mut libc::iovec;
	// This is not a no-op on all platforms.
	#[allow(clippy::useless_conversion)]
	{
		header.msg_iovlen = buffer.len().try_into()
			.map_err(|_| std::io::ErrorKind::InvalidInput)?;
	}
	header.msg_flags = 0;
	header.msg_control = control_data;
	// This is not a no-op on all platforms.
	#[allow(clippy::useless_conversion)]
	{
		header.msg_controllen = ancillary.capacity().try_into()
			.map_err(|_| std::io::ErrorKind::InvalidInput)?;
	}

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
