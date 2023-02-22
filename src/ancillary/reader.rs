use std::os::fd::{OwnedFd, BorrowedFd};

use super::FD_SIZE;

/// Reader to parse received ancillary messages from a Unix socket.
///
/// # Example
/// ```no_run
/// use tokio_seqpacket::UnixSeqpacket;
/// use tokio_seqpacket::ancillary::{AncillaryMessageReader, AncillaryMessage};
/// use std::io::IoSliceMut;
/// use std::os::fd::AsRawFd;
///
/// #[tokio::main]
/// async fn main() -> std::io::Result<()> {
///     let sock = UnixSeqpacket::connect("/tmp/sock").await?;
///
///     let mut fds = [0; 8];
///     let mut ancillary_buffer = [0; 128];
///     let mut ancillary = AncillaryMessageReader::new(&mut ancillary_buffer);
///
///     let mut buf = [1; 8];
///     let mut bufs = [IoSliceMut::new(&mut buf)];
///     sock.recv_vectored_with_ancillary(&mut bufs, &mut ancillary).await?;
///
///     for message in ancillary.messages() {
///         if let AncillaryMessage::FileDescriptors(fds) = message {
///             for fd in fds {
///                 println!("received file descriptor: {}", fd.as_raw_fd());
///             }
///         }
///     }
///     Ok(())
/// }
/// ```
#[derive(Debug)]
pub struct AncillaryMessageReader<'a> {
	pub(crate) buffer: &'a mut [u8],
	pub(crate) length: usize,
	pub(crate) truncated: bool,
}

/// Iterator over ancillary messages from a [`AncillaryMessageReader`].
#[derive(Copy, Clone)]
pub struct AncillaryMessages<'a> {
	buffer: &'a [u8],
	current: Option<&'a libc::cmsghdr>,
}

/// Owning iterator over ancillary messages from a [`AncillaryMessageReader`].
pub struct IntoAncillaryMessages<'a> {
	buffer: &'a mut [u8],
	current: Option<&'a libc::cmsghdr>,
}

/// This enum represent one control message of variable type.
pub enum AncillaryMessage<'a> {
	/// Ancillary message holding file descriptors.
	FileDescriptors(FileDescriptors<'a>),

	/// Ancillary message holding unix credentials.
	#[cfg(any(doc, target_os = "android", target_os = "linux", target_os = "netbsd",))]
	Credentials(UnixCredentials<'a>),

	/// Ancillary message uninterpreted data.
	Other(UnknownMessage<'a>)
}

/// This enum represent one control message of variable type.
///
/// Where applicable, it has taken ownership of the objects in the control message.
pub enum OwnedAncillaryMessage<'a> {
	/// Ancillary message holding file descriptors.
	FileDescriptors(OwnedFileDescriptors<'a>),

	/// Ancillary message holding unix credentials.
	#[cfg(any(doc, target_os = "android", target_os = "linux", target_os = "netbsd",))]
	Credentials(UnixCredentials<'a>),

	/// Ancillary message uninterpreted data.
	Other(UnknownMessage<'a>)
}

/// A control message containing borrowed file descriptors.
#[derive(Copy, Clone)]
pub struct FileDescriptors<'a> {
	/// The message data.
	data: &'a [u8],
}

/// A control message containing owned file descriptors.
pub struct OwnedFileDescriptors<'a> {
	/// The message data.
	data: &'a mut [u8],
	position: usize,
}

/// A control message containing unix credentials for a process.
#[derive(Copy, Clone)]
#[cfg(any(target_os = "linux", target_os = "android", target_os = "netbsd"))]
pub struct UnixCredentials<'a> {
	/// The message data.
	data: &'a [u8],
}

/// An unrecognized control message.
#[derive(Copy, Clone)]
pub struct UnknownMessage<'a> {
	/// The `cmsg_level` field of the ancillary data.
	cmsg_level: i32,

	/// The `cmsg_type` field of the ancillary data.
	cmsg_type: i32,

	/// The message data.
	data: &'a [u8],
}

impl<'a> AncillaryMessageReader<'a> {
	/// Create an ancillary data with the given buffer.
	///
	/// # Example
	///
	/// ```no_run
	/// # #![allow(unused_mut)]
	/// use tokio_seqpacket::ancillary::AncillaryMessageReader;
	/// let mut ancillary_buffer = [0; 128];
	/// let mut ancillary = AncillaryMessageReader::new(&mut ancillary_buffer);
	/// ```
	pub fn new(buffer: &'a mut [u8]) -> Self {
		Self { buffer, length: 0, truncated: false }
	}

	/// Returns the capacity of the buffer.
	pub fn capacity(&self) -> usize {
		self.buffer.len()
	}

	/// Returns `true` if the ancillary data is empty.
	pub fn is_empty(&self) -> bool {
		self.length == 0
	}

	/// Returns the number of used bytes.
	pub fn len(&self) -> usize {
		self.length
	}

	/// Is `true` if during a recv operation the ancillary message was truncated.
	///
	/// # Example
	///
	/// ```no_run
	/// use tokio_seqpacket::UnixSeqpacket;
	/// use tokio_seqpacket::ancillary::AncillaryMessageReader;
	/// use std::io::IoSliceMut;
	///
	/// #[tokio::main]
	/// async fn main() -> std::io::Result<()> {
	///     let sock = UnixSeqpacket::connect("/tmp/sock").await?;
	///
	///     let mut ancillary_buffer = [0; 128];
	///     let mut ancillary = AncillaryMessageReader::new(&mut ancillary_buffer);
	///
	///     let mut buf = [1; 8];
	///     let mut bufs = &mut [IoSliceMut::new(&mut buf)];
	///     sock.recv_vectored_with_ancillary(bufs, &mut ancillary).await?;
	///
	///     println!("Is truncated: {}", ancillary.is_truncated());
	///     Ok(())
	/// }
	/// ```
	pub fn is_truncated(&self) -> bool {
		self.truncated
	}

	/// Returns the iterator of the control messages.
	pub fn messages(&self) -> AncillaryMessages<'_> {
		AncillaryMessages { buffer: &self.buffer[..self.length], current: None }
	}

	/// Consume the ancillary message to take ownership of the file descriptors.
	///
	/// Note that file descriptors added by [`Self::add_fds()`] are not owned by this struct,
	/// and are not returned by this function.
	/// Only file descriptors added by [`Self::add_owned_fds()`] and file descriptors received from the OS are returned.
	pub fn into_messages(mut self) -> IntoAncillaryMessages<'a> {
		let buffer = std::mem::take(&mut self.buffer);
		let length = std::mem::take(&mut self.length);
		IntoAncillaryMessages { buffer: &mut buffer[..length], current: None }
	}
}

impl Drop for AncillaryMessageReader<'_> {
	fn drop(&mut self) {
		if self.length > 0 {
			drop(IntoAncillaryMessages { buffer: &mut self.buffer[..self.length], current: None })
		}
	}
}

impl<'a> Iterator for AncillaryMessages<'a> {
	type Item = AncillaryMessage<'a>;

	fn next(&mut self) -> Option<Self::Item> {
		if self.buffer.is_empty() {
			return None;
		}
		unsafe {
			let mut msg: libc::msghdr = std::mem::zeroed();
			msg.msg_control = self.buffer.as_ptr() as *mut _;
			msg.msg_controllen = self.buffer.len() as _;

			let cmsg = if let Some(current) = self.current {
				libc::CMSG_NXTHDR(&msg, current)
			} else {
				libc::CMSG_FIRSTHDR(&msg)
			};

			let cmsg = cmsg.as_ref()?;

			// Most operating systems, but not Linux or emscripten, return the previous pointer
			// when its length is zero. Therefore, check if the previous pointer is the same as
			// the current one.
			if let Some(current) = self.current {
				if std::ptr::eq(current, cmsg) {
					return None;
				}
			}

			self.current = Some(cmsg);
			let ancillary_result = AncillaryMessage::try_from_cmsghdr(cmsg);
			Some(ancillary_result)
		}
	}
}

impl<'a> AncillaryMessage<'a> {
	#[allow(clippy::unnecessary_cast)]
	fn try_from_cmsghdr(cmsg: &'a libc::cmsghdr) -> Self {
		unsafe {
			let cmsg_len_zero = libc::CMSG_LEN(0) as usize;
			let data_len = cmsg.cmsg_len as usize - cmsg_len_zero;
			let data = libc::CMSG_DATA(cmsg).cast();
			let data = std::slice::from_raw_parts(data, data_len);

			match (cmsg.cmsg_level, cmsg.cmsg_type) {
				(libc::SOL_SOCKET, libc::SCM_RIGHTS) => Self::FileDescriptors(FileDescriptors { data }),
				#[cfg(any(target_os = "android", target_os = "linux",))]
				(libc::SOL_SOCKET, libc::SCM_CREDENTIALS) => Self::Credentials(UnixCredentials { data }),
				#[cfg(target_os = "netbsd")]
				(libc::SOL_SOCKET, libc::SCM_CREDS) => Self::Credentials(UnixCredentials { data }),
				(cmsg_level, cmsg_type) => Self::Other(UnknownMessage { cmsg_level, cmsg_type, data }),
			}
		}
	}
}

impl<'a> Iterator for IntoAncillaryMessages<'a> {
	type Item = OwnedAncillaryMessage<'a>;

	fn next(&mut self) -> Option<Self::Item> {
		if self.buffer.is_empty() {
			return None;
		}
		unsafe {
			let mut msg: libc::msghdr = std::mem::zeroed();
			msg.msg_control = self.buffer.as_ptr() as *mut _;
			msg.msg_controllen = self.buffer.len() as _;

			let cmsg = if let Some(current) = self.current {
				libc::CMSG_NXTHDR(&msg, current)
			} else {
				libc::CMSG_FIRSTHDR(&msg)
			};

			let cmsg = cmsg.as_ref()?;

			// Most operating systems, but not Linux or emscripten, return the previous pointer
			// when its length is zero. Therefore, check if the previous pointer is the same as
			// the current one.
			if let Some(current) = self.current {
				if std::ptr::eq(current, cmsg) {
					return None;
				}
			}

			self.current = Some(cmsg);
			let ancillary_result = OwnedAncillaryMessage::try_from_cmsghdr(cmsg);
			Some(ancillary_result)
		}
	}
}

impl Drop for IntoAncillaryMessages<'_> {
	fn drop(&mut self) {
		for message in self {
			drop(message)
		}
	}
}

impl<'a> OwnedAncillaryMessage<'a> {
	#[allow(clippy::unnecessary_cast)]
	fn try_from_cmsghdr(cmsg: &'a libc::cmsghdr) -> Self {
		unsafe {
			let cmsg_len_zero = libc::CMSG_LEN(0) as usize;
			let data_len = cmsg.cmsg_len as usize - cmsg_len_zero;
			let data = libc::CMSG_DATA(cmsg).cast();
			let data = std::slice::from_raw_parts_mut(data, data_len);

			match (cmsg.cmsg_level, cmsg.cmsg_type) {
				(libc::SOL_SOCKET, libc::SCM_RIGHTS) => Self::FileDescriptors(OwnedFileDescriptors { data, position: 0 }),
				#[cfg(any(target_os = "android", target_os = "linux",))]
				(libc::SOL_SOCKET, libc::SCM_CREDENTIALS) => Self::Credentials(UnixCredentials { data }),
				#[cfg(target_os = "netbsd")]
				(libc::SOL_SOCKET, libc::SCM_CREDS) => Self::Credentials(UnixCredentials { data }),
				(cmsg_level, cmsg_type) => Self::Other(UnknownMessage { cmsg_level, cmsg_type, data }),
			}
		}
	}
}

impl<'a> FileDescriptors<'a> {
	/// Get the number of file descriptors in the message.
	pub fn len(&self) -> usize {
		self.data.len() / FD_SIZE
	}

	/// Check if the message is empty (contains no file descriptors).
	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}

	/// Get a borrowed file descriptor from the message.
	///
	/// Returns `None` if the index is out of bounds.
	pub fn get(&self, index: usize) -> Option<BorrowedFd<'a>> {
		if index >= self.len() {
			None
		} else {
			// SAFETY: The memory is valid, and the kernel guaranteed it is a file descriptor.
			// Additionally, the returned lifetime is linked to the `AncillaryMessageReader` which owns the file descriptor.
			unsafe {
				Some(std::ptr::read_unaligned(self.data[index * FD_SIZE..].as_ptr().cast()))
			}
		}
	}
}

impl<'a> Iterator for FileDescriptors<'a> {
	type Item = BorrowedFd<'a>;

	fn next(&mut self) -> Option<Self::Item> {
		let fd = self.get(0)?;
		self.data = &self.data[FD_SIZE..];
		Some(fd)
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		(self.len(), Some(self.len()))
	}
}

impl<'a> std::iter::ExactSizeIterator for FileDescriptors<'a> {
	fn len(&self) -> usize {
		self.len()
	}
}

impl<'a> OwnedFileDescriptors<'a> {
	/// Get the number of file descriptors in the message.
	pub fn len(&self) -> usize {
		self.data[self.position..].len() / FD_SIZE
	}

	/// Check if the message is empty (contains no file descriptors).
	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}

	/// Take ownership of a specific file descriptor in the message.
	///
	/// Returns `None` if ownership of the file descriptor has already been taken,
	/// or if the index is out of bounds.
	pub fn take_ownership(&mut self, index: usize) -> Option<OwnedFd> {
		if index >= self.len() {
			None
		} else {
			// SAFETY: The memory is valid, and the kernel guaranteed it is a file descriptor.
			// Additionally, the returned lifetime is linked to the `AncillaryMessageReader` which owns the file descriptor.
			// And we overwrite the original value with -1 before returning the owned fd to ensure we don't try to own it multiple times.
			unsafe {
				use std::os::fd::{FromRawFd, RawFd};
				let ptr = self.data[index * FD_SIZE..].as_mut_ptr().cast();
				let raw_fd: RawFd = std::ptr::read_unaligned(ptr);
				if raw_fd == -1 {
					None
				} else {
					std::ptr::write_unaligned(ptr, -1);
					Some(OwnedFd::from_raw_fd(raw_fd))
				}
			}
		}
	}
}

impl<'a> Iterator for OwnedFileDescriptors<'a> {
	type Item = OwnedFd;

	fn next(&mut self) -> Option<Self::Item> {
		while !Self::is_empty(self) {
			let fd = self.take_ownership(self.position);
			self.position += 1;
			if let Some(fd) = fd {
				return Some(fd)
			}
		}
		None
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		(self.len(), Some(self.len()))
	}
}

impl Drop for OwnedFileDescriptors<'_> {
	fn drop(&mut self) {
		for fd in self {
			drop(fd)
		}
	}
}

impl<'a> std::iter::ExactSizeIterator for OwnedFileDescriptors<'a> {
	fn len(&self) -> usize {
		self.len()
	}
}

#[cfg(any(target_os = "linux", target_os = "android", target_os = "netbsd"))]
mod unix_creds_impl {
	use super::UnixCredentials;
	use super::super::{SocketCred, CREDS_SIZE };

	impl UnixCredentials<'_> {
		/// Get the number of credentials in the message.
		pub fn len(&self) -> usize {
			self.data.len() / CREDS_SIZE
		}

		/// Check if the message is empty (contains no credentials).
		pub fn is_empty(&self) -> bool {
			self.len() == 0
		}

		/// Get the credentials at a specific index.
		pub fn get(&self, index: usize) -> Option<SocketCred> {
			if index >= self.len() {
				None
			} else {
				// SAFETY: The memory is valid, and the kernel guaranteed it is a credentials struct.
				// It probably also guarantees alignment, but just in case not, use read_unaligned.
				unsafe {
					Some(std::ptr::read_unaligned(self.data[index * CREDS_SIZE..].as_ptr().cast()))
				}
			}
		}
	}

	impl Iterator for UnixCredentials<'_> {
		type Item = SocketCred;

		fn next(&mut self) -> Option<Self::Item> {
			let fd = self.get(0)?;
			self.data = &self.data[CREDS_SIZE..];
			Some(fd)
		}

		fn size_hint(&self) -> (usize, Option<usize>) {
			(self.len(), Some(self.len()))
		}
	}

	impl<'a> std::iter::ExactSizeIterator for UnixCredentials<'a> {
		fn len(&self) -> usize {
			self.len()
		}
	}
}

impl<'a> UnknownMessage<'a> {
	/// Get the cmsg_level of the message.
	pub fn cmsg_level(&self) -> i32 {
		self.cmsg_level
	}

	/// Get the cmsg_type of the message.
	pub fn cmsg_type(&self) -> i32 {
		self.cmsg_type
	}

	/// Get the data of the message.
	pub fn data(&self) -> &'a [u8] {
		self.data
	}
}
