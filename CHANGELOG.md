v0.4.0
  * Make I/O functions take `&self` instead of `&mut self`.
  * Deprecate the `split()` API.

v0.3.0:
  * Update to tokio 0.3.2.
  * Report peer credentials with own `UCred` type since tokio made the construction private.

v0.2.1:
  * Fix receiving of ancillary data.

v0.2.0:
  * Add supported for vectored I/O.
  * Add support for ancillary data.
  * Allow sockets to be split in a read half and a write half.
