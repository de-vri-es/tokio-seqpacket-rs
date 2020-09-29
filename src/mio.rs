use ::mio::event::Evented;
use ::mio::unix::EventedFd;
use ::mio::{Poll, PollOpt, Ready, Token};
use std::os::unix::io::{AsRawFd, RawFd};

/// Wrapper around [`socket2::Socket`] that implements [`mio::event::Evented`].
pub struct EventedSocket {
	inner: socket2::Socket,
}

impl EventedSocket {
	pub fn new(inner: socket2::Socket) -> Self {
		Self { inner }
	}

	pub fn inner(&self) -> &socket2::Socket {
		&self.inner
	}

	pub fn inner_mut(&mut self) -> &mut socket2::Socket {
		&mut self.inner
	}
}

impl std::ops::Deref for EventedSocket {
	type Target = socket2::Socket;

	fn deref(&self) -> &Self::Target {
		self.inner()
	}
}

impl std::ops::DerefMut for EventedSocket {
	fn deref_mut(&mut self) -> &mut Self::Target {
		self.inner_mut()
	}
}

impl AsRawFd for EventedSocket {
	fn as_raw_fd(&self) -> RawFd {
		self.inner.as_raw_fd()
	}
}

impl Evented for EventedSocket {
	fn register(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt) -> std::io::Result<()> {
		EventedFd(&self.as_raw_fd()).register(poll, token, interest, opts)
	}

	fn reregister(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt) -> std::io::Result<()> {
		EventedFd(&self.as_raw_fd()).reregister(poll, token, interest, opts)
	}

	fn deregister(&self, poll: &Poll) -> std::io::Result<()> {
		EventedFd(&self.as_raw_fd()).deregister(poll)
	}
}
