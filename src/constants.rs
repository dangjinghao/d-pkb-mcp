//! Shared numeric parameters. Equal values with different meanings stay separate.

pub(crate) const IO_BUFFER_BYTES: usize = 65_536;
pub(crate) const TOKEN_BYTES: usize = 32;
pub(crate) const DEFAULT_LINK_TTL_SECS: u64 = 300;
pub(crate) const MIN_LINK_TTL_SECS: u64 = 1;
pub(crate) const MAX_LINK_TTL_SECS: u64 = 3_600;
pub(crate) const DEFAULT_TRANSFER_QUOTA_BYTES: u64 = 1_073_741_824;
