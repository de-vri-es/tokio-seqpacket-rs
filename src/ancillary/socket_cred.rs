#[cfg(all(doc, not(target_os = "android"), not(target_os = "linux"), not(target_os = "netbsd")))]
#[derive(Copy, Clone)]
pub struct SocketCred(());

/// Unix credentials.
#[cfg(any(target_os = "android", target_os = "linux"))]
#[derive(Copy, Clone)]
pub struct SocketCred(libc::ucred);

/// Unix credentials.
#[cfg(target_os = "netbsd")]
#[derive(Copy, Clone)]
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
