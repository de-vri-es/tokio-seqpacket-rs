//! Support for creating / parsing ancillary data.

// Copied from PR to the standard library.
// PR: https://github.com/rust-lang/rust/pull/69864
// File downloaded from: https://github.com/rust-lang/rust/blob/3eb5c4581a386b13c414e8c8bd73846ef37236d1/library/std/src/os/unix/net/ancillary.rs

use std::marker::PhantomData;
use std::mem::{size_of, zeroed};
use std::os::fd::BorrowedFd;
use std::os::unix::io::RawFd;
use std::ptr::read_unaligned;
use std::slice::from_raw_parts;

fn add_to_ancillary_data<T>(
	buffer: &mut [u8],
	length: &mut usize,
	source: &[T],
	cmsg_level: libc::c_int,
	cmsg_type: libc::c_int,
) -> bool {
	let source_len = if let Some(source_len) = source.len().checked_mul(size_of::<T>()) {
		if let Ok(source_len) = u32::try_from(source_len) {
			source_len
		} else {
			return false;
		}
	} else {
		return false;
	};

	unsafe {
		let additional_space = libc::CMSG_SPACE(source_len) as usize;

		let new_length = if let Some(new_length) = additional_space.checked_add(*length) {
			new_length
		} else {
			return false;
		};

		if new_length > buffer.len() {
			return false;
		}

		buffer[*length..new_length].fill(0);

		*length = new_length;

		let mut msg: libc::msghdr = zeroed();
		msg.msg_control = buffer.as_mut_ptr().cast();
		msg.msg_controllen = *length as _;

		let mut cmsg = libc::CMSG_FIRSTHDR(&msg);
		let mut previous_cmsg = cmsg;
		while !cmsg.is_null() {
			previous_cmsg = cmsg;
			cmsg = libc::CMSG_NXTHDR(&msg, cmsg);

			// Most operating systems, but not Linux or emscripten, return the previous pointer
			// when its length is zero. Therefore, check if the previous pointer is the same as
			// the current one.
			if std::ptr::eq(cmsg, previous_cmsg) {
				break;
			}
		}

		if previous_cmsg.is_null() {
			return false;
		}

		(*previous_cmsg).cmsg_level = cmsg_level;
		(*previous_cmsg).cmsg_type = cmsg_type;
		(*previous_cmsg).cmsg_len = libc::CMSG_LEN(source_len) as _;

		let data = libc::CMSG_DATA(previous_cmsg).cast();

		libc::memcpy(data, source.as_ptr().cast(), source_len as usize);
	}
	true
}

struct AncillaryDataIter<'a, T> {
	data: &'a [u8],
	phantom: PhantomData<T>,
}

impl<'a, T> AncillaryDataIter<'a, T> {
	/// Create `AncillaryDataIter` struct to iterate through the data unit in the control message.
	///
	/// # Safety
	///
	/// `data` must contain a valid control message.
	unsafe fn new(data: &'a [u8]) -> AncillaryDataIter<'a, T> {
		AncillaryDataIter { data, phantom: PhantomData }
	}
}

impl<'a, T> Iterator for AncillaryDataIter<'a, T> {
	type Item = T;

	fn next(&mut self) -> Option<T> {
		if size_of::<T>() <= self.data.len() {
			unsafe {
				let unit = read_unaligned(self.data.as_ptr().cast());
				self.data = &self.data[size_of::<T>()..];
				Some(unit)
			}
		} else {
			None
		}
	}
}

#[cfg(all(doc, not(target_os = "android"), not(target_os = "linux"), not(target_os = "netbsd")))]
#[derive(Clone)]
pub struct SocketCred(());

/// Unix credential.
#[cfg(any(target_os = "android", target_os = "linux",))]
#[derive(Clone)]
pub struct SocketCred(libc::ucred);

#[cfg(target_os = "netbsd")]
#[derive(Clone)]
pub struct SocketCred(libc::sockcred);

#[cfg(any(target_os = "android", target_os = "linux"))]
impl SocketCred {
	/// Create a Unix credential struct.
	///
	/// PID, UID and GID is set to 0.
	#[allow(clippy::new_without_default)]
	pub fn new() -> SocketCred {
		SocketCred(libc::ucred { pid: 0, uid: 0, gid: 0 })
	}

	/// Set the PID.
	pub fn set_pid(&mut self, pid: libc::pid_t) {
		self.0.pid = pid;
	}

	/// Get the current PID.
	pub fn get_pid(&self) -> libc::pid_t {
		self.0.pid
	}

	/// Set the UID.
	pub fn set_uid(&mut self, uid: libc::uid_t) {
		self.0.uid = uid;
	}

	/// Get the current UID.
	#[must_use]
	pub fn get_uid(&self) -> libc::uid_t {
		self.0.uid
	}

	/// Set the GID.
	pub fn set_gid(&mut self, gid: libc::gid_t) {
		self.0.gid = gid;
	}

	/// Get the current GID.
	#[must_use]
	pub fn get_gid(&self) -> libc::gid_t {
		self.0.gid
	}
}

#[cfg(target_os = "netbsd")]
impl SocketCred {
	/// Create a Unix credential struct.
	///
	/// PID, UID and GID is set to 0.
	#[allow(clippy::new_without_default)]
	pub fn new() -> SocketCred {
		SocketCred(libc::sockcred {
			sc_pid: 0,
			sc_uid: 0,
			sc_euid: 0,
			sc_gid: 0,
			sc_egid: 0,
			sc_ngroups: 0,
			sc_groups: [0u32; 1],
		})
	}

	/// Set the PID.
	pub fn set_pid(&mut self, pid: libc::pid_t) {
		self.0.sc_pid = pid;
	}

	/// Get the current PID.
	pub fn get_pid(&self) -> libc::pid_t {
		self.0.sc_pid
	}

	/// Set the UID.
	pub fn set_uid(&mut self, uid: libc::uid_t) {
		self.0.sc_uid = uid;
	}

	/// Get the current UID.
	pub fn get_uid(&self) -> libc::uid_t {
		self.0.sc_uid
	}

	/// Set the GID.
	pub fn set_gid(&mut self, gid: libc::gid_t) {
		self.0.sc_gid = gid;
	}

	/// Get the current GID.
	pub fn get_gid(&self) -> libc::gid_t {
		self.0.sc_gid
	}
}

/// This control message contains file descriptors.
///
/// The level is equal to `SOL_SOCKET` and the type is equal to `SCM_RIGHTS`.
pub struct ScmRights<'a>(AncillaryDataIter<'a, RawFd>);

impl<'a> Iterator for ScmRights<'a> {
	type Item = RawFd;

	fn next(&mut self) -> Option<RawFd> {
		self.0.next()
	}
}

#[cfg(all(doc, not(target_os = "android"), not(target_os = "linux"), not(target_os = "netbsd")))]
pub struct ScmCredentials<'a>(AncillaryDataIter<'a, ()>);

/// This control message contains unix credentials.
///
/// The level is equal to `SOL_SOCKET` and the type is equal to `SCM_CREDENTIALS` or `SCM_CREDS`.
#[cfg(any(target_os = "android", target_os = "linux",))]
pub struct ScmCredentials<'a>(AncillaryDataIter<'a, libc::ucred>);

#[cfg(target_os = "netbsd")]
pub struct ScmCredentials<'a>(AncillaryDataIter<'a, libc::sockcred>);

#[cfg(any(doc, target_os = "android", target_os = "linux", target_os = "netbsd",))]
impl<'a> Iterator for ScmCredentials<'a> {
	type Item = SocketCred;

	fn next(&mut self) -> Option<SocketCred> {
		Some(SocketCred(self.0.next()?))
	}
}

/// The error type which is returned from parsing the type a control message.
#[non_exhaustive]
#[derive(Debug)]
pub enum AncillaryError {
	/// The ancillary data type is not recognized.
	Unknown {
		/// The cmsg_level field of the ancillary data.
		cmsg_level: i32,

		/// The cmsg_type field of the ancillary data.
		cmsg_type: i32,
	},
}

/// This enum represent one control message of variable type.
pub enum AncillaryData<'a> {
	/// Ancillary data holding file descriptors.
	ScmRights(ScmRights<'a>),

	/// Ancillary data holding unix credentials.
	#[cfg(any(doc, target_os = "android", target_os = "linux", target_os = "netbsd",))]
	ScmCredentials(ScmCredentials<'a>),
}

impl<'a> AncillaryData<'a> {
	/// Create an `AncillaryData::ScmRights` variant.
	///
	/// # Safety
	///
	/// `data` must contain a valid control message and the control message must be type of
	/// `SOL_SOCKET` and level of `SCM_RIGHTS`.
	unsafe fn as_rights(data: &'a [u8]) -> Self {
		let ancillary_data_iter = AncillaryDataIter::new(data);
		let scm_rights = ScmRights(ancillary_data_iter);
		AncillaryData::ScmRights(scm_rights)
	}

	/// Create an `AncillaryData::ScmCredentials` variant.
	///
	/// # Safety
	///
	/// `data` must contain a valid control message and the control message must be type of
	/// `SOL_SOCKET` and level of `SCM_CREDENTIALS` or `SCM_CREDS`.
	#[cfg(any(doc, target_os = "android", target_os = "linux", target_os = "netbsd",))]
	unsafe fn as_credentials(data: &'a [u8]) -> Self {
		let ancillary_data_iter = AncillaryDataIter::new(data);
		let scm_credentials = ScmCredentials(ancillary_data_iter);
		AncillaryData::ScmCredentials(scm_credentials)
	}

	#[allow(clippy::unnecessary_cast)]
	fn try_from_cmsghdr(cmsg: &'a libc::cmsghdr) -> Result<Self, AncillaryError> {
		unsafe {
			let cmsg_len_zero = libc::CMSG_LEN(0) as usize;
			let data_len = cmsg.cmsg_len as usize - cmsg_len_zero;
			let data = libc::CMSG_DATA(cmsg).cast();
			let data = from_raw_parts(data, data_len);

			match cmsg.cmsg_level {
				libc::SOL_SOCKET => match cmsg.cmsg_type {
					libc::SCM_RIGHTS => Ok(AncillaryData::as_rights(data)),
					#[cfg(any(target_os = "android", target_os = "linux",))]
					libc::SCM_CREDENTIALS => Ok(AncillaryData::as_credentials(data)),
					#[cfg(target_os = "netbsd")]
					libc::SCM_CREDS => Ok(AncillaryData::as_credentials(data)),
					cmsg_type => {
						Err(AncillaryError::Unknown { cmsg_level: libc::SOL_SOCKET, cmsg_type })
					}
				},
				cmsg_level => {
					Err(AncillaryError::Unknown { cmsg_level, cmsg_type: cmsg.cmsg_type })
				}
			}
		}
	}
}

/// This struct is used to iterate through the control messages.
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct Messages<'a> {
	buffer: &'a [u8],
	current: Option<&'a libc::cmsghdr>,
}

impl<'a> Iterator for Messages<'a> {
	type Item = Result<AncillaryData<'a>, AncillaryError>;

	fn next(&mut self) -> Option<Self::Item> {
		unsafe {
			let mut msg: libc::msghdr = zeroed();
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
			let ancillary_result = AncillaryData::try_from_cmsghdr(cmsg);
			Some(ancillary_result)
		}
	}
}

/// A Unix socket Ancillary data struct.
///
/// # Example
/// ```no_run
/// use tokio_seqpacket::UnixSeqpacket;
/// use tokio_seqpacket::ancillary::{SocketAncillary, AncillaryData};
/// use std::io::IoSliceMut;
///
/// #[tokio::main]
/// async fn main() -> std::io::Result<()> {
///     let sock = UnixSeqpacket::connect("/tmp/sock").await?;
///
///     let mut fds = [0; 8];
///     let mut ancillary_buffer = [0; 128];
///     let mut ancillary = SocketAncillary::new(&mut ancillary_buffer[..]);
///
///     let mut buf = [1; 8];
///     let mut bufs = &mut [IoSliceMut::new(&mut buf[..])][..];
///     sock.recv_vectored_with_ancillary(bufs, &mut ancillary).await?;
///
///     for ancillary_result in ancillary.messages() {
///         if let AncillaryData::ScmRights(scm_rights) = ancillary_result.unwrap() {
///             for fd in scm_rights {
///                 println!("receive file descriptor: {fd}");
///             }
///         }
///     }
///     Ok(())
/// }
/// ```
#[derive(Debug)]
pub struct SocketAncillary<'a> {
	pub(crate) buffer: &'a mut [u8],
	pub(crate) length: usize,
	pub(crate) truncated: bool,
}

impl<'a> SocketAncillary<'a> {
	/// Create an ancillary data with the given buffer.
	///
	/// # Example
	///
	/// ```no_run
	/// # #![allow(unused_mut)]
	/// use tokio_seqpacket::ancillary::SocketAncillary;
	/// let mut ancillary_buffer = [0; 128];
	/// let mut ancillary = SocketAncillary::new(&mut ancillary_buffer[..]);
	/// ```
	pub fn new(buffer: &'a mut [u8]) -> Self {
		SocketAncillary { buffer, length: 0, truncated: false }
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

	/// Returns the iterator of the control messages.
	pub fn messages(&self) -> Messages<'_> {
		Messages { buffer: &self.buffer[..self.length], current: None }
	}

	/// Is `true` if during a recv operation the ancillary was truncated.
	///
	/// # Example
	///
	/// ```no_run
	/// use tokio_seqpacket::UnixSeqpacket;
	/// use tokio_seqpacket::ancillary::SocketAncillary;
	/// use std::io::IoSliceMut;
	///
	/// #[tokio::main]
	/// async fn main() -> std::io::Result<()> {
	///     let sock = UnixSeqpacket::connect("/tmp/sock").await?;
	///
	///     let mut ancillary_buffer = [0; 128];
	///     let mut ancillary = SocketAncillary::new(&mut ancillary_buffer[..]);
	///
	///     let mut buf = [1; 8];
	///     let mut bufs = &mut [IoSliceMut::new(&mut buf[..])][..];
	///     sock.recv_vectored_with_ancillary(bufs, &mut ancillary).await?;
	///
	///     println!("Is truncated: {}", ancillary.truncated());
	///     Ok(())
	/// }
	/// ```
	pub fn truncated(&self) -> bool {
		self.truncated
	}

	/// Add file descriptors to the ancillary data.
	///
	/// The function returns `true` if there was enough space in the buffer.
	/// If there was not enough space then no file descriptors was appended.
	/// Technically, that means this operation adds a control message with the level `SOL_SOCKET`
	/// and type `SCM_RIGHTS`.
	///
	/// # Example
	///
	/// ```no_run
	/// use tokio_seqpacket::UnixSeqpacket;
	/// use tokio_seqpacket::ancillary::SocketAncillary;
	/// use std::os::unix::io::AsFd;
	/// use std::io::IoSlice;
	///
	/// #[tokio::main]
	/// async fn main() -> std::io::Result<()> {
	///     let sock = UnixSeqpacket::connect("/tmp/sock").await?;
	///
	///     let mut ancillary_buffer = [0; 128];
	///     let mut ancillary = SocketAncillary::new(&mut ancillary_buffer[..]);
	///     ancillary.add_fds(&[sock.as_fd()][..]);
	///
	///     let buf = [1; 8];
	///     let mut bufs = &mut [IoSlice::new(&buf[..])][..];
	///     sock.send_vectored_with_ancillary(bufs, &mut ancillary).await?;
	///     Ok(())
	/// }
	/// ```
	pub fn add_fds(&mut self, fds: &[BorrowedFd<'a>]) -> bool {
		self.truncated = false;
		add_to_ancillary_data(
			self.buffer,
			&mut self.length,
			fds,
			libc::SOL_SOCKET,
			libc::SCM_RIGHTS,
		)
	}

	/// Add credentials to the ancillary data.
	///
	/// The function returns `true` if there was enough space in the buffer.
	/// If there was not enough space then no credentials was appended.
	/// Technically, that means this operation adds a control message with the level `SOL_SOCKET`
	/// and type `SCM_CREDENTIALS` or `SCM_CREDS`.
	///
	#[cfg(any(doc, target_os = "android", target_os = "linux", target_os = "netbsd",))]
	pub fn add_creds(&mut self, creds: &[SocketCred]) -> bool {
		self.truncated = false;
		add_to_ancillary_data(
			self.buffer,
			&mut self.length,
			creds,
			libc::SOL_SOCKET,
			#[cfg(not(target_os = "netbsd"))]
			libc::SCM_CREDENTIALS,
			#[cfg(target_os = "netbsd")]
			libc::SCM_CREDS,
		)
	}

	/// Clears the ancillary data, removing all values.
	///
	/// # Example
	///
	/// ```no_run
	/// use tokio_seqpacket::UnixSeqpacket;
	/// use tokio_seqpacket::ancillary::{SocketAncillary, AncillaryData};
	/// use std::io::IoSliceMut;
	///
	/// #[tokio::main]
	/// async fn main() -> std::io::Result<()> {
	///     let sock = UnixSeqpacket::connect("/tmp/sock").await?;
	///
	///     let mut fds1 = [0; 8];
	///     let mut fds2 = [0; 8];
	///     let mut ancillary_buffer = [0; 128];
	///     let mut ancillary = SocketAncillary::new(&mut ancillary_buffer[..]);
	///
	///     let mut buf = [1; 8];
	///     let mut bufs = &mut [IoSliceMut::new(&mut buf[..])][..];
	///
	///     sock.recv_vectored_with_ancillary(bufs, &mut ancillary).await?;
	///     for ancillary_result in ancillary.messages() {
	///         if let AncillaryData::ScmRights(scm_rights) = ancillary_result.unwrap() {
	///             for fd in scm_rights {
	///                 println!("receive file descriptor: {fd}");
	///             }
	///         }
	///     }
	///
	///     ancillary.clear();
	///
	///     sock.recv_vectored_with_ancillary(bufs, &mut ancillary).await?;
	///     for ancillary_result in ancillary.messages() {
	///         if let AncillaryData::ScmRights(scm_rights) = ancillary_result.unwrap() {
	///             for fd in scm_rights {
	///                 println!("receive file descriptor: {fd}");
	///             }
	///         }
	///     }
	///     Ok(())
	/// }
	/// ```
	pub fn clear(&mut self) {
		self.length = 0;
		self.truncated = false;
	}
}
