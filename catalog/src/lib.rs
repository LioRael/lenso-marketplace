//! Marketplace persistence; shared wire verification belongs to the catalog protocol.
#[cfg(feature = "native")]
pub mod cache;
#[cfg(feature = "native")]
pub mod directory;
pub mod persistence;
pub use lenso_plugin_catalog::*;
