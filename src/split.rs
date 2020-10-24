use futures::future::poll_fn;
use std::io::{IoSlice, IoSliceMut};
use std::task::{Context, Poll};

use crate::ancillary::SocketAncillary;
use crate::UnixSeqpacket;

/// The read half of a seqpacket socket.
pub struct ReadHalf<'a>(&'a UnixSeqpacket);

/// The write half of a seqpacket socket.
pub struct WriteHalf<'a>(&'a UnixSeqpacket);

impl<'a> ReadHalf<'a> {
	/// Create a read half from a reference to a UnixSeqpacket.
	///
	/// # Safety
	/// You must ensure that only one read half is created and that the original socket is not used for reading any more.
	pub(crate) unsafe fn new(parent: &'a UnixSeqpacket) -> Self {
		Self(parent)
	}

	/// Get the socket address of the local half of this connection.
	pub fn local_addr(&self) -> std::io::Result<std::os::unix::net::SocketAddr> {
		self.0.local_addr()
	}

	/// Get the socket address of the remote half of this connection.
	pub fn peer_addr(&self) -> std::io::Result<std::os::unix::net::SocketAddr> {
		self.0.peer_addr()
	}

	/// Get the effective credentials of the process which called `connect` or `pair`.
	pub fn peer_cred(&self) -> std::io::Result<tokio::net::unix::UCred> {
		self.0.peer_cred()
	}

	/// Try to receive data on the socket from the connected peer without blocking.
	///
	/// If there is no data ready yet, the current task is scheduled to wake up when the socket becomes readable.
	pub fn poll_recv(&mut self, cx: &mut Context, buffer: &mut [u8]) -> Poll<std::io::Result<usize>> {
		crate::socket::poll_recv(&self.0, cx, buffer)
	}

	/// Try to receive data on the socket from the connected peer without blocking.
	///
	/// If there is no data ready yet, the current task is scheduled to wake up when the socket becomes readable.
	pub fn poll_recv_vectored(&mut self, cx: &mut Context, buffer: &mut [IoSliceMut]) -> Poll<std::io::Result<usize>> {
		crate::socket::poll_recv_vectored(&self.0, cx, buffer)
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
		crate::socket::poll_recv_vectored_with_ancillary(&self.0, cx, buffer, ancillary)
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
	pub fn shutdown(&self) -> std::io::Result<()> {
		self.0.shutdown(std::net::Shutdown::Read)
	}
}

impl<'a> WriteHalf<'a> {
	/// Create a write half from a reference to a UnixSeqpacket.
	///
	/// # Safety
	/// You must ensure that only one write half is created and that the original socket is not used for writing any more.
	pub(crate) unsafe fn new(parent: &'a UnixSeqpacket) -> Self {
		Self(parent)
	}

	/// Get the socket address of the local half of this connection.
	pub fn local_addr(&self) -> std::io::Result<std::os::unix::net::SocketAddr> {
		self.0.local_addr()
	}

	/// Get the socket address of the remote half of this connection.
	pub fn peer_addr(&self) -> std::io::Result<std::os::unix::net::SocketAddr> {
		self.0.peer_addr()
	}

	/// Get the effective credentials of the process which called `connect` or `pair`.
	pub fn peer_cred(&self) -> std::io::Result<tokio::net::unix::UCred> {
		self.0.peer_cred()
	}

	/// Shuts down the write halve of the connection.
	pub fn shutdown(&self) -> std::io::Result<()> {
		self.0.shutdown(std::net::Shutdown::Read)
	}

	/// Try to send data on the socket to the connected peer without blocking.
	///
	/// If the socket is not ready yet, the current task is scheduled to wake up when the socket becomes writeable.
	pub fn poll_send(&mut self, cx: &mut Context, buffer: &[u8]) -> Poll<std::io::Result<usize>> {
		crate::socket::poll_send(&self.0, cx, buffer)
	}

	/// Try to send data on the socket to the connected peer without blocking.
	///
	/// If the socket is not ready yet, the current task is scheduled to wake up when the socket becomes writeable.
	pub fn poll_send_vectored(&mut self, cx: &mut Context, buffer: &[IoSlice]) -> Poll<std::io::Result<usize>> {
		crate::socket::poll_send_vectored(&self.0, cx, buffer)
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
		crate::socket::poll_send_vectored_with_ancillary(&self.0, cx, buffer, ancillary)
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
}
