//! GUI-free core of the native NFL play viewer: schema, read-only data access,
//! filters with faceted counts, role-based stats, the frame blob codec, field
//! geometry, the team table and style tokens. Nothing here depends on egui.

pub mod blob;
pub mod dal;
pub mod filter;
pub mod geometry;
pub mod labels;
pub mod roster;
pub mod schema;
pub mod stats;
pub mod style;
pub mod teams;
pub mod usmap;

/// Error type shared by the crate. Kept as a boxed error so SQLite, I/O and
/// decoding errors can all flow through `?`.
pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, Error>;
