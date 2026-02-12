use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use maxminddb::Reader;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::io::Write;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

// Custom deserialization structs optimized for geoipsed use case.
//
// Trade-off vs built-in maxminddb::geoip2 structs:
//
// PROS (why we use Fast* structs):
// - Skip unnecessary fields (postal, subdivisions, traits, registered_country, represented_country)
// - Use owned String instead of borrowed &str, avoiding lifetime complexities
// - Simpler field access without nested Option unwrapping
// - Slightly faster deserialization by ignoring unused fields
//
// CONS (what we give up):
// - Extra allocations for owned strings (vs zero-copy borrows in geoip2 structs)
// - Manual maintenance if MMDB schema changes
// - Code duplication with upstream structs
//
// In 0.27+, the built-in geoip2::City and geoip2::Asn structs improved significantly
// (Names struct replaces BTreeMap, Default on nested fields), but they use lifetimes
// tied to LookupResult which complicates our template rendering. The Fast* structs
// provide a good balance of simplicity and performance for our use case.

#[derive(Deserialize)]
struct FastAsn {
    autonomous_system_number: Option<u32>,
    autonomous_system_organization: Option<String>,
}

#[derive(Deserialize)]
struct FastCity {
    city: Option<FastNames>,
    continent: Option<FastCode>,
    country: Option<FastCountry>,
    location: Option<FastLocation>,
}

#[derive(Deserialize)]
struct FastNames {
    names: Option<FastNamesMap>,
}

#[derive(Deserialize)]
struct FastNamesMap {
    en: Option<String>,
}

#[derive(Deserialize)]
struct FastCode {
    code: Option<String>,
}

#[derive(Deserialize)]
struct FastCountry {
    iso_code: Option<String>,
    names: Option<FastNamesMap>,
}

#[derive(Deserialize)]
struct FastLocation {
    latitude: Option<f64>,
    longitude: Option<f64>,
    time_zone: Option<String>,
}

/// Represents a field that can be used in templates
#[derive(Debug, Clone, Serialize)]
pub struct TemplateField {
    /// Name of the field as used in templates
    pub name: String,
    /// Human-readable description of the field
    pub description: String,
    /// Example value for documentation
    pub example: String,
}

/// Trait for MMDB providers that can extract data from IP addresses
pub trait MmdbProvider: fmt::Debug {
    /// Get the name of this provider
    fn name(&self) -> &str;

    /// Get the default search path for this provider's database files
    fn default_path(&self) -> PathBuf;

    /// Get a list of database files this provider needs
    fn required_files(&self) -> Vec<String>;

    /// Get a list of fields available for use in templates
    fn available_fields(&self) -> Vec<TemplateField>;

    /// Check if all required database files are available
    fn check_files(&self, path: &Path) -> Result<()> {
        // For backward compatibility and testing, we'll look for one of the required files
        // instead of requiring all of them
        let mut found_one = false;

        for file in self.required_files() {
            let file_path = path.join(&file);
            if file_path.exists() {
                found_one = true;
                break;
            }
        }

        if !found_one {
            anyhow::bail!(
                "No database files found in {}. Need at least one of: {:?}. \
                Try setting GEOIP_MMDB_DIR environment variable to your database directory.",
                path.display(),
                self.required_files()
            );
        }

        Ok(())
    }

    /// Initialize this provider with the given path
    fn initialize(&mut self, path: &Path) -> Result<()>;

    /// Lookup data for an IP address and format it according to the template
    fn lookup(
        &self,
        ip: IpAddr,
        ip_str: &str,
        template: &crate::template::Template,
    ) -> Result<String>;

    /// Lookup data for an IP address and write it directly to a writer
    fn lookup_and_write(
        &self,
        wtr: &mut dyn std::io::Write,
        ip: IpAddr,
        ip_str: &str,
        template: &crate::template::Template,
    ) -> Result<()>;

    /// Checks if an IP address has a valid ASN entry (used for routability check)
    fn has_asn(&self, ip: IpAddr) -> bool;
}

/// Provider for MaxMind GeoIP2 databases
#[derive(Debug)]
pub struct MaxMindProvider {
    name: String,
    initialized: bool,
    asn_reader: Option<Reader<maxminddb::Mmap>>,
    city_reader: Option<Reader<maxminddb::Mmap>>,
    ipv4_reader: Option<Reader<maxminddb::Mmap>>,
    ipv6_reader: Option<Reader<maxminddb::Mmap>>,
}

impl Default for MaxMindProvider {
    fn default() -> Self {
        Self {
            name: "MaxMind GeoIP2".to_string(),
            initialized: false,
            asn_reader: None,
            city_reader: None,
            ipv4_reader: None,
            ipv6_reader: None,
        }
    }
}

impl MaxMindProvider {
    /// Helper to lookup ASN data for an IP address.
    /// Returns None if the database isn't available or the lookup fails.
    fn lookup_asn(&self, ip: IpAddr) -> Option<FastAsn> {
        let is_ipv4 = matches!(ip, IpAddr::V4(_));

        if let Some(ref asn_reader) = self.asn_reader {
            return asn_reader
                .lookup(ip)
                .ok()
                .and_then(|lookup| lookup.decode::<FastAsn>().ok().flatten());
        }

        // Try version-specific readers
        let reader = if is_ipv4 {
            &self.ipv4_reader
        } else {
            &self.ipv6_reader
        };
        reader
            .as_ref()
            .and_then(|r| r.lookup(ip).ok())
            .and_then(|lookup| lookup.decode::<FastAsn>().ok().flatten())
    }

    /// Helper to lookup City data for an IP address.
    /// Returns None if the database isn't available or the lookup fails.
    fn lookup_city(&self, ip: IpAddr) -> Option<FastCity> {
        self.city_reader
            .as_ref()
            .and_then(|r| r.lookup(ip).ok())
            .and_then(|lookup| lookup.decode::<FastCity>().ok().flatten())
    }

    /// Core template rendering logic shared by lookup() and lookup_and_write().
    /// Performs database lookups and writes formatted output to the provided writer.
    fn render_template(
        &self,
        wtr: &mut dyn Write,
        ip: IpAddr,
        ip_str: &str,
        template: &crate::template::Template,
    ) -> Result<()> {
        // Lookup data from databases
        let asn_record = self.lookup_asn(ip);
        let city_record = self.lookup_city(ip);

        // Reusable buffers for number formatting (avoids allocations)
        let mut asn_num_buf = itoa::Buffer::new();
        let mut lat_buf = ryu::Buffer::new();
        let mut lon_buf = ryu::Buffer::new();

        template.write(wtr, |out, field| {
            let val = match field {
                "ip" => ip_str,
                "asnnum" => {
                    let asn_num = asn_record
                        .as_ref()
                        .and_then(|r| r.autonomous_system_number)
                        .unwrap_or(0);
                    asn_num_buf.format(asn_num)
                }
                "asnorg" => asn_record
                    .as_ref()
                    .and_then(|r| r.autonomous_system_organization.as_deref())
                    .unwrap_or(""),
                "city" => city_record
                    .as_ref()
                    .and_then(|r| r.city.as_ref())
                    .and_then(|c| c.names.as_ref())
                    .and_then(|n| n.en.as_deref())
                    .unwrap_or(""),
                "continent" => city_record
                    .as_ref()
                    .and_then(|r| r.continent.as_ref())
                    .and_then(|c| c.code.as_deref())
                    .unwrap_or(""),
                "country_iso" => city_record
                    .as_ref()
                    .and_then(|r| r.country.as_ref())
                    .and_then(|c| c.iso_code.as_deref())
                    .unwrap_or(""),
                "country_full" => city_record
                    .as_ref()
                    .and_then(|r| r.country.as_ref())
                    .and_then(|c| c.names.as_ref())
                    .and_then(|n| n.en.as_deref())
                    .unwrap_or(""),
                "latitude" => {
                    let val = city_record
                        .as_ref()
                        .and_then(|r| r.location.as_ref())
                        .and_then(|l| l.latitude)
                        .unwrap_or(0.0);
                    lat_buf.format(val)
                }
                "longitude" => {
                    let val = city_record
                        .as_ref()
                        .and_then(|r| r.location.as_ref())
                        .and_then(|l| l.longitude)
                        .unwrap_or(0.0);
                    lon_buf.format(val)
                }
                "timezone" => city_record
                    .as_ref()
                    .and_then(|r| r.location.as_ref())
                    .and_then(|l| l.time_zone.as_deref())
                    .unwrap_or(""),
                _ => "",
            };

            // Replace spaces with underscores to avoid breaking column alignment in logs
            if val.contains(' ') {
                out.write_all(val.replace(' ', "_").as_bytes())
            } else {
                out.write_all(val.as_bytes())
            }
        })?;

        Ok(())
    }
}

impl MmdbProvider for MaxMindProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn default_path(&self) -> PathBuf {
        // Look for standard directories in this order:
        // 1. /usr/share/GeoIP
        // 2. /opt/homebrew/var/GeoIP
        // 3. /var/lib/GeoIP
        let paths = vec![
            PathBuf::from("/usr/share/GeoIP"),
            PathBuf::from("/opt/homebrew/var/GeoIP"),
            PathBuf::from("/var/lib/GeoIP"),
        ];

        for path in paths {
            if path.exists() {
                return path;
            }
        }

        // Default to /usr/share/GeoIP if none found
        PathBuf::from("/usr/share/GeoIP")
    }

    fn required_files(&self) -> Vec<String> {
        // List all possible file patterns
        vec![
            // Standard unified databases
            "GeoLite2-ASN.mmdb".to_string(),
            "GeoLite2-City.mmdb".to_string(),
            // Separate IPv4/IPv6 databases
            "GeoLite2-ASN-IPv4.mmdb".to_string(),
            "GeoLite2-ASN-IPv6.mmdb".to_string(),
            "GeoLite2-City-IPv4.mmdb".to_string(),
            "GeoLite2-City-IPv6.mmdb".to_string(),
        ]
    }

    fn available_fields(&self) -> Vec<TemplateField> {
        vec![
            TemplateField {
                name: "ip".to_string(),
                description: "The IP address itself".to_string(),
                example: "93.184.216.34".to_string(),
            },
            TemplateField {
                name: "asnnum".to_string(),
                description: "Autonomous System Number".to_string(),
                example: "15133".to_string(),
            },
            TemplateField {
                name: "asnorg".to_string(),
                description: "Autonomous System Organization".to_string(),
                example: "MCI Communications Services".to_string(),
            },
            TemplateField {
                name: "city".to_string(),
                description: "City name".to_string(),
                example: "Los Angeles".to_string(),
            },
            TemplateField {
                name: "continent".to_string(),
                description: "Continent code".to_string(),
                example: "NA".to_string(),
            },
            TemplateField {
                name: "country_iso".to_string(),
                description: "Country ISO code".to_string(),
                example: "US".to_string(),
            },
            TemplateField {
                name: "country_full".to_string(),
                description: "Full country name".to_string(),
                example: "United States".to_string(),
            },
            TemplateField {
                name: "latitude".to_string(),
                description: "Latitude coordinate".to_string(),
                example: "34.0544".to_string(),
            },
            TemplateField {
                name: "longitude".to_string(),
                description: "Longitude coordinate".to_string(),
                example: "-118.2441".to_string(),
            },
            TemplateField {
                name: "timezone".to_string(),
                description: "Time zone name".to_string(),
                example: "America/Los_Angeles".to_string(),
            },
        ]
    }

    fn initialize(&mut self, path: &Path) -> Result<()> {
        // Try to open the main databases first (normal operation)
        let asn_path = path.join("GeoLite2-ASN.mmdb");
        if asn_path.exists() {
            self.asn_reader = Some(unsafe { Reader::open_mmap(&asn_path) }.with_context(|| {
                format!("Failed to open ASN database at {}", asn_path.display())
            })?);
        }

        let city_path = path.join("GeoLite2-City.mmdb");
        if city_path.exists() {
            self.city_reader =
                Some(unsafe { Reader::open_mmap(&city_path) }.with_context(|| {
                    format!("Failed to open City database at {}", city_path.display())
                })?);
        }

        // Try to open separate IPv4/IPv6 databases if main ones aren't available
        if self.asn_reader.is_none() {
            let ipv4_asn_path = path.join("GeoLite2-ASN-IPv4.mmdb");
            let ipv6_asn_path = path.join("GeoLite2-ASN-IPv6.mmdb");

            if ipv4_asn_path.exists() {
                self.ipv4_reader = Some(
                    unsafe { Reader::open_mmap(&ipv4_asn_path) }.with_context(|| {
                        format!(
                            "Failed to open IPv4 ASN database at {}",
                            ipv4_asn_path.display()
                        )
                    })?,
                );
            }

            if ipv6_asn_path.exists() {
                self.ipv6_reader = Some(
                    unsafe { Reader::open_mmap(&ipv6_asn_path) }.with_context(|| {
                        format!(
                            "Failed to open IPv6 ASN database at {}",
                            ipv6_asn_path.display()
                        )
                    })?,
                );
            }
        }

        if self.city_reader.is_none() {
            let ipv4_city_path = path.join("GeoLite2-City-IPv4.mmdb");
            let ipv6_city_path = path.join("GeoLite2-City-IPv6.mmdb");

            if ipv4_city_path.exists() {
                self.ipv4_reader = Some(
                    unsafe { Reader::open_mmap(&ipv4_city_path) }.with_context(|| {
                        format!(
                            "Failed to open IPv4 City database at {}",
                            ipv4_city_path.display()
                        )
                    })?,
                );
            }

            if ipv6_city_path.exists() {
                self.ipv6_reader = Some(
                    unsafe { Reader::open_mmap(&ipv6_city_path) }.with_context(|| {
                        format!(
                            "Failed to open IPv6 City database at {}",
                            ipv6_city_path.display()
                        )
                    })?,
                );
            }
        }

        // Ensure we have at least one database available
        if self.asn_reader.is_none()
            && self.city_reader.is_none()
            && self.ipv4_reader.is_none()
            && self.ipv6_reader.is_none()
        {
            // In production, we need actual readers
            anyhow::bail!("No valid MMDB databases found in {}", path.display());
        }

        self.initialized = true;
        Ok(())
    }

    fn lookup(
        &self,
        ip: IpAddr,
        ip_str: &str,
        template: &crate::template::Template,
    ) -> Result<String> {
        if !self.initialized {
            anyhow::bail!("Provider not initialized");
        }

        let mut buf = Vec::with_capacity(64);
        self.render_template(&mut buf, ip, ip_str, template)?;

        let result = String::from_utf8(buf).unwrap_or_default();
        Ok(result.replace(' ', "_"))
    }

    fn lookup_and_write(
        &self,
        wtr: &mut dyn std::io::Write,
        ip: IpAddr,
        ip_str: &str,
        template: &crate::template::Template,
    ) -> Result<()> {
        if !self.initialized {
            anyhow::bail!("Provider not initialized");
        }

        self.render_template(wtr, ip, ip_str, template)
    }

    fn has_asn(&self, ip: IpAddr) -> bool {
        if !self.initialized {
            return false;
        }

        // Check ASN reader
        if let Some(ref asn_reader) = self.asn_reader {
            return asn_reader.lookup(ip).is_ok();
        }

        // Check IPv4/IPv6 specific readers
        let is_ipv4 = matches!(ip, IpAddr::V4(_));
        let reader = if is_ipv4 {
            &self.ipv4_reader
        } else {
            &self.ipv6_reader
        };

        if let Some(ref reader) = reader {
            return reader.lookup(ip).is_ok();
        }

        false
    }
}

/// Registry of available MMDB providers
#[derive(Debug)]
pub struct ProviderRegistry {
    providers: std::collections::HashMap<String, Box<dyn MmdbProvider>>,
    active_provider: Option<String>,
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        let mut registry = Self {
            providers: std::collections::HashMap::new(),
            active_provider: None,
        };

        // Register default providers
        registry.register("maxmind".to_string(), Box::new(MaxMindProvider::default()));

        // Set MaxMind as the default active provider
        registry.active_provider = Some("maxmind".to_string());

        registry
    }
}

impl ProviderRegistry {
    /// Register a new provider
    pub fn register(&mut self, name: String, provider: Box<dyn MmdbProvider>) {
        self.providers.insert(name, provider);
    }

    /// Get a list of available provider names
    pub fn available_providers(&self) -> Vec<String> {
        self.providers.keys().cloned().collect()
    }

    /// Set the active provider
    pub fn set_active_provider(&mut self, name: &str) -> Result<()> {
        if self.providers.contains_key(name) {
            self.active_provider = Some(name.to_string());
            Ok(())
        } else {
            anyhow::bail!("Unknown provider: {}", name)
        }
    }

    /// Get the active provider, taking ownership of it
    pub fn get_active_provider_owned(&mut self) -> Result<Box<dyn MmdbProvider>> {
        let name = self
            .active_provider
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No active provider set"))?;
        self.providers
            .remove(name)
            .ok_or_else(|| anyhow::anyhow!("Active provider '{}' not found in registry", name))
    }

    /// Get the active provider
    pub fn get_active_provider(&self) -> Result<&dyn MmdbProvider> {
        let name = self
            .active_provider
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No active provider set"))?;

        self.providers
            .get(name)
            .map(|p| p.as_ref())
            .ok_or_else(|| anyhow::anyhow!("Active provider not found"))
    }

    /// Apply a function to the active provider, returning the result
    pub fn with_active_provider_mut<F, T>(&mut self, f: F) -> Result<T>
    where
        F: FnOnce(&mut dyn MmdbProvider) -> Result<T>,
    {
        let name = self
            .active_provider
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No active provider set"))?
            .clone();

        let provider = self
            .providers
            .get_mut(&name)
            .ok_or_else(|| anyhow::anyhow!("Active provider not found"))?;

        f(provider.as_mut())
    }

    /// Initialize the active provider with the given path
    pub fn initialize_active_provider(&mut self, path: Option<Utf8PathBuf>) -> Result<()> {
        let active_name = self
            .active_provider
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No active provider set"))?
            .clone();

        let default_path = if let Some(provider) = self.providers.get(&active_name) {
            provider.default_path()
        } else {
            return Err(anyhow::anyhow!("Active provider not found"));
        };

        let path_to_use = path
            .map(|p| PathBuf::from(p.as_str()))
            .unwrap_or_else(|| default_path);

        self.with_active_provider_mut(|provider| provider.initialize(&path_to_use))
    }

    /// Lookup data for an IP address using the active provider
    pub fn lookup(
        &self,
        ip: IpAddr,
        ip_str: &str,
        template: &crate::template::Template,
    ) -> Result<String> {
        self.get_active_provider()?.lookup(ip, ip_str, template)
    }

    /// Check if an IP has a valid ASN entry using the active provider
    pub fn has_asn(&self, ip: IpAddr) -> bool {
        if let Ok(provider) = self.get_active_provider() {
            return provider.has_asn(ip);
        }
        false
    }

    /// Get available fields for the active provider
    pub fn available_fields(&self) -> Result<Vec<TemplateField>> {
        let provider = self.get_active_provider()?;
        Ok(provider.available_fields())
    }

    /// Print information about database files for all providers
    pub fn print_db_info(&self) -> Result<String> {
        let mut output = String::new();

        // Check environment variables
        let env_var_status = match std::env::var("GEOIP_MMDB_DIR") {
            Ok(path) => format!("GEOIP_MMDB_DIR is set to: {}", path),
            Err(_) => match std::env::var("MAXMIND_MMDB_DIR") {
                Ok(path) => format!(
                    "MAXMIND_MMDB_DIR is set to: {} (deprecated, use GEOIP_MMDB_DIR instead)",
                    path
                ),
                Err(_) => "No GEOIP_MMDB_DIR environment variable set".to_string(),
            },
        };

        output.push_str(&format!("Environment Status:\n  {}\n\n", env_var_status));
        output.push_str("Available MMDB Providers:\n\n");

        for (name, provider) in &self.providers {
            output.push_str(&format!("Provider: {}\n", name));
            output.push_str(&format!("  Name: {}\n", provider.name()));
            output.push_str(&format!(
                "  Default Path: {}\n",
                provider.default_path().display()
            ));
            output.push_str("  Required Files:\n");

            for file in provider.required_files() {
                output.push_str(&format!("    - {}\n", file));
            }

            let default_path = provider.default_path();
            let files_exist = provider
                .required_files()
                .iter()
                .any(|f| default_path.join(f).exists());

            if files_exist {
                // Green checkmark with ANSI color code
                output.push_str(
                    "  Status: \x1b[32m✓\x1b[0m Installed (files found at default location)\n",
                );
            } else {
                // Red X with ANSI color code
                output
                    .push_str("  Status: \x1b[31m✗\x1b[0m Not installed or custom path required\n");
            }

            output.push('\n');
        }

        Ok(output)
    }
}
