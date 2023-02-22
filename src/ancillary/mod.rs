//! Support for reading / writing ancillary data.

use std::os::fd::BorrowedFd;

mod reader;
pub use reader::*;

mod writer;
pub use writer::{AncillaryMessageWriter, AddControlMessageError};

#[cfg(any(doc, target_os = "linux", target_os = "android", target_os = "netbsd"))]
mod socket_cred;

#[cfg(any(doc, target_os = "linux", target_os = "android", target_os = "netbsd"))]
pub use socket_cred::SocketCred;

const FD_SIZE: usize = std::mem::size_of::<BorrowedFd>();

#[cfg(any(doc, target_os = "linux", target_os = "android", target_os = "netbsd"))]
const CREDS_SIZE: usize = std::mem::size_of::<SocketCred>();
