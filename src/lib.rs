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
//! use std::path::{Path, PathBuf};
//! use anyhow::Result;
//!
//! // Implement the MmdbProvider trait for your custom provider
//! #[derive(Debug)]
//! struct CustomProvider {
//!     name: String,
//!     initialized: bool,
//! }
//!
//! impl MmdbProvider for CustomProvider {
//!     fn name(&self) -> &str {
//!         &self.name
//!     }
//!
//!     fn default_path(&self) -> PathBuf {
//!         PathBuf::from("/path/to/mmdb")
//!     }
//!
//!     fn required_files(&self) -> Vec<String> {
//!         vec!["custom.mmdb".to_string()]
//!     }
//!
//!     fn available_fields(&self) -> Vec<TemplateField> {
//!         vec![
//!             TemplateField {
//!                 name: "ip".to_string(),
//!                 description: "The IP address".to_string(),
//!                 example: "93.184.216.34".to_string(),
//!             },
//!         ]
//!     }
//!
//!     fn initialize(&mut self, _path: &Path) -> Result<()> {
//!         self.initialized = true;
//!         Ok(())
//!     }
//!
//!     fn lookup(&self, ip_str: &str, template: &str) -> Result<String> {
//!         // Your lookup implementation here
//!         Ok(template.replace("{ip}", ip_str))
//!     }
//!
//!     fn has_asn(&self, _ip_str: &str) -> bool {
//!         false
//!     }
//! }
//!
//! // Register with a registry
//! let mut registry = ProviderRegistry::default();
//! registry.register(
//!     "custom".to_string(),
//!     Box::new(CustomProvider {
//!         name: "My Custom Provider".to_string(),
//!         initialized: false,
//!     })
//! );
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
