use libc::{gid_t, pid_t, uid_t};
use std::os::unix::io::AsRawFd;

/// Credentials of a process
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct UCred {
	/// PID (process ID) of the process
	pid: Option<pid_t>,
	/// UID (user ID) of the process
	uid: uid_t,
	/// GID (group ID) of the process
	gid: gid_t,
}

impl UCred {
	/// Get the credentials of a connected peer for a Unix socket.
	pub fn from_socket_peer<T: AsRawFd>(socket: &T) -> std::io::Result<Self> {
		get_peer_cred(socket)
	}

	/// Gets UID (user ID) of the process.
	pub fn uid(&self) -> uid_t {
		self.uid
	}

	/// Gets GID (group ID) of the process.
	pub fn gid(&self) -> gid_t {
		self.gid
	}

	/// Gets PID (process ID) of the process.
	///
	/// This is only implemented under Linux, Android, iOS, macOS, Solaris and
	/// Illumos. On other plaforms this will always return `None`.
	pub fn pid(&self) -> Option<pid_t> {
		self.pid
	}
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn get_peer_cred<T: AsRawFd>(sock: &T) -> std::io::Result<UCred> {
	use libc::{c_void, getsockopt, socklen_t, ucred, SOL_SOCKET, SO_PEERCRED};

	let raw_fd = sock.as_raw_fd();

	let mut ucred = ucred {
		pid: 0,
		uid: 0,
		gid: 0,
	};

	let mut ucred_size = std::mem::size_of::<ucred>() as socklen_t;

	let ret = unsafe {
		getsockopt(
			raw_fd,
			SOL_SOCKET,
			SO_PEERCRED,
			&mut ucred as *mut ucred as *mut c_void,
			&mut ucred_size,
		)
	};

	if ret == 0 {
		Ok(UCred {
			uid: ucred.uid,
			gid: ucred.gid,
			pid: Some(ucred.pid),
		})
	} else {
		Err(std::io::Error::last_os_error())
	}
}

#[cfg(any(
	target_os = "dragonfly",
	target_os = "freebsd",
	target_os = "netbsd",
	target_os = "openbsd"
))]
fn get_peer_cred<T: AsRawFd>(sock: &T) -> std::io::Result<UCred> {
	let raw_fd = sock.as_raw_fd();

	let mut uid = 0;
	let mut gid = 0;

	let ret = unsafe {
		getpeereid(raw_fd, uid.as_mut_ptr(), gid.as_mut_ptr());
	};

	if ret == 0 {
		Ok(UCred {
			uid,
			gid,
			pid: None,
		})
	} else {
		Err(std::io::Error::last_os_error())
	}
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn get_peer_cred<T: AsRawFd>(sock: &T) -> std::io::Result<UCred> {
	let raw_fd = sock.as_raw_fd();

	let mut uid = 0 as uid_t;
	let mut gid = 0 as gid_t;
	let mut pid = 0 as pid_t;
	let mut pid_size = size_of::<pid_t>() as u32;

	let ret = unsafe {
		getsockopt(
			raw_fd,
			SOL_LOCAL,
			LOCAL_PEEREPID,
			pid.as_mut_ptr() as *mut c_void,
			pid_size.as_mut_ptr(),
		)
	};
	if ret != 0 {
		return Err(std::io::Error::last_os_error());
	}

	let ret = unsafe {
		getpeereid(raw_fd, uid.as_mut_ptr(), gid.as_mut_ptr())
	};

	if ret == 0 {
		Ok(UCred {
			uid,
			gid,
			pid: Some(pid),
		})
	} else {
		Err(std::io::Error::last_os_error())
	}
}

#[cfg(any(target_os = "solaris", target_os = "illumos"))]
fn get_peer_cred<T: AsRawFd>(sock: &T) -> std::io::Result<UCred> {
	let raw_fd = sock.as_raw_fd();
	let mut cred = std::ptr::null_mut();
	unsafe {
		let ret = libc::getpeerucred(raw_fd, &mut cred);

		if ret == 0 {
			let uid = libc::ucred_geteuid(cred);
			let gid = libc::ucred_getegid(cred);
			let pid = libc::ucred_getpid(cred);

			libc::ucred_free(cred);

			Ok(UCred {
				uid,
				gid,
				pid: Some(pid),
			})
		} else {
			Err(std::io::Error::last_os_error())
		}
	}
}
