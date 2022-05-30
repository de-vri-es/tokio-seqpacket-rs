main:
  * Add `as_async_fd()` to facilitate low level access to the file descriptor.

v0.5.4:
  * Fix sending ancillary data on non-Linux platforms.
  * Fix building of documentation on non-Linux platforms.

v0.5.3
  * Update dependencies.

v0.5.2
  * Add conversions to/from raw FDs for `UnixSeqpacketListener`.
  * Remove `socket2` dependency.
  * Fix compilation for several BSD targets.

v0.5.1
  * Upgrade to `socket2` 0.4.

v0.5.0
  * Report socket address as `PathBuf`.
  * Remove `UnixSeqpacket::local/remote_addr`, as they never contain useful information.

v0.4.5
  * Properly allow multiple tasks to call async function on the same socket (poll functions still only wake the last task).
  * Fix potential hang in `UnixSeqpacketListener::accept()`.

v0.4.4
  * Fix potential hangs in I/O functions.

v0.4.3
  * Fix compilation for `musl` targets.
  * Add conversions to/from raw file descriptors.

v0.4.2
  * Fix links in `README.md`.

v0.4.1
  * Regenerate `README.md` from library documentation.

v0.4.0
  * Make I/O functions take `&self` instead of `&mut self`.
  * Deprecate the `split()` API.

v0.3.1:
  * Fix potential hangs in I/O functions (backported from 0.4.4).

v0.3.0:
  * Update to tokio 0.3.2.
  * Report peer credentials with own `UCred` type since tokio made the construction private.

v0.2.1:
  * Fix receiving of ancillary data.

v0.2.0:
  * Add supported for vectored I/O.
  * Add support for ancillary data.
  * Allow sockets to be split in a read half and a write half.
