//! The geoipsed library for IP address extraction and geolocation tagging.
//!
//! This library provides functionality for finding and decorating IP addresses
//! with geolocation information from various MMDB databases.
//!
//! # Examples
//!
//! Creating and registering a custom MMDB provider:
//!
//! ```rust,no_run
//! use geoipsed::{MmdbProvider, ProviderRegistry, TemplateField};
//! use geoipsed::{define_mmdb_provider, register_mmdb_provider};
//!
//! // Create a custom provider with the macro
//! let provider = define_mmdb_provider!(
//!     name: "My Custom Provider",
//!     default_path: "/path/to/mmdb",
//!     files: ["custom.mmdb"],
//!     fields: [
//!         "ip" => {
//!             description: "The IP address",
//!             example: "93.184.216.34"
//!         },
//!         "custom_field" => {
//!             description: "My custom field",
//!             example: "Custom value"
//!         }
//!     ]
//! );
//!
//! // Register with a registry
//! let mut registry = ProviderRegistry::default();
//! register_mmdb_provider!(registry, "mycustom", provider);
//! ```

pub mod error;
pub mod extractor;
pub mod files;
pub mod geoip;
pub mod input;
pub mod mmdb;
pub mod tag;
pub mod template;

/// Re-export key traits for custom MMDB format support
pub use crate::error::Error;
pub use crate::mmdb::{MmdbProvider, ProviderRegistry, TemplateField};
