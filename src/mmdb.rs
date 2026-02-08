use std::fmt;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use maxminddb::{geoip2, Mmap, Reader};
use serde::Serialize;

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
    fn lookup(&self, ip: &str, template: &str) -> Result<String>;

    /// Checks if an IP address has a valid ASN entry (used for routability check)
    fn has_asn(&self, ip: &str) -> bool;
}

/// Provider for MaxMind GeoIP2 databases
#[derive(Debug)]
pub struct MaxMindProvider {
    name: String,
    initialized: bool,
    asn_reader: Option<Reader<Mmap>>,
    city_reader: Option<Reader<Mmap>>,
    ipv4_reader: Option<Reader<Mmap>>,
    ipv6_reader: Option<Reader<Mmap>>,
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
            self.asn_reader = Some(Reader::open_mmap(&asn_path).with_context(|| {
                format!("Failed to open ASN database at {}", asn_path.display())
            })?);
        }

        let city_path = path.join("GeoLite2-City.mmdb");
        if city_path.exists() {
            self.city_reader = Some(Reader::open_mmap(&city_path).with_context(|| {
                format!("Failed to open City database at {}", city_path.display())
            })?);
        }

        // Try to open separate IPv4/IPv6 databases if main ones aren't available
        if self.asn_reader.is_none() {
            let ipv4_asn_path = path.join("GeoLite2-ASN-IPv4.mmdb");
            let ipv6_asn_path = path.join("GeoLite2-ASN-IPv6.mmdb");

            if ipv4_asn_path.exists() {
                self.ipv4_reader = Some(Reader::open_mmap(&ipv4_asn_path).with_context(|| {
                    format!(
                        "Failed to open IPv4 ASN database at {}",
                        ipv4_asn_path.display()
                    )
                })?);
            }

            if ipv6_asn_path.exists() {
                self.ipv6_reader = Some(Reader::open_mmap(&ipv6_asn_path).with_context(|| {
                    format!(
                        "Failed to open IPv6 ASN database at {}",
                        ipv6_asn_path.display()
                    )
                })?);
            }
        }

        if self.city_reader.is_none() {
            let ipv4_city_path = path.join("GeoLite2-City-IPv4.mmdb");
            let ipv6_city_path = path.join("GeoLite2-City-IPv6.mmdb");

            if ipv4_city_path.exists() {
                self.ipv4_reader = Some(Reader::open_mmap(&ipv4_city_path).with_context(|| {
                    format!(
                        "Failed to open IPv4 City database at {}",
                        ipv4_city_path.display()
                    )
                })?);
            }

            if ipv6_city_path.exists() {
                self.ipv6_reader = Some(Reader::open_mmap(&ipv6_city_path).with_context(|| {
                    format!(
                        "Failed to open IPv6 City database at {}",
                        ipv6_city_path.display()
                    )
                })?);
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

    fn lookup(&self, ip_str: &str, template: &str) -> Result<String> {
        use crate::template::Template;

        if !self.initialized {
            anyhow::bail!("Provider not initialized");
        }

        // Parse the IP address
        let ip: IpAddr = ip_str.parse().context("Invalid IP address")?;

        // Choose the appropriate reader based on IP version
        let is_ipv4 = matches!(ip, IpAddr::V4(_));

        // Get ASN information
        let mut asn_num_val = 0u32;
        let mut asn_org_val = "";

        if let Some(ref asn_reader) = self.asn_reader {
            if let Ok(asn_record) = asn_reader.lookup::<geoip2::Asn>(ip) {
                if let Some(asn) = asn_record.autonomous_system_number {
                    asn_num_val = asn;
                }
                if let Some(org) = asn_record.autonomous_system_organization {
                    asn_org_val = org;
                }
            }
        } else if (is_ipv4 && self.ipv4_reader.is_some())
            || (!is_ipv4 && self.ipv6_reader.is_some())
        {
            // If we have separate IPv4/IPv6 readers, use the appropriate one
            let reader = if is_ipv4 {
                &self.ipv4_reader
            } else {
                &self.ipv6_reader
            };
            if let Some(ref reader) = reader {
                if let Ok(asn_record) = reader.lookup::<geoip2::Asn>(ip) {
                    if let Some(asn) = asn_record.autonomous_system_number {
                        asn_num_val = asn;
                    }
                    if let Some(org) = asn_record.autonomous_system_organization {
                        asn_org_val = org;
                    }
                }
            }
        }

        let asnnum = if asn_num_val == 0 {
            "0".to_string()
        } else {
            asn_num_val.to_string()
        };
        let asnorg = asn_org_val.to_string();

        // Get City/Country information
        let mut continent_val = "";
        let mut country_iso_val = "";
        let mut country_full_val = "";
        let mut city_val = "";
        let mut timezone_val = "";
        let mut lat_val = 0.0;
        let mut lon_val = 0.0;

        if let Some(ref city_reader) = self.city_reader {
            if let Ok(city_record) = city_reader.lookup::<geoip2::City>(ip) {
                // Continent info
                if let Some(c) = city_record.continent.and_then(|c| c.code) {
                    continent_val = c;
                }

                // Country info
                if let Some(country) = city_record.country {
                    if let Some(iso) = country.iso_code {
                        country_iso_val = iso;
                    }
                    if let Some(names) = country.names {
                        if let Some(name) = names.get("en") {
                            country_full_val = *name;
                        }
                    }
                }

                // City info
                if let Some(c) = city_record.city.and_then(|c| c.names) {
                    if let Some(name) = c.get("en") {
                        city_val = *name;
                    }
                }

                // Location info
                if let Some(location) = city_record.location {
                    if let Some(tz) = location.time_zone {
                        timezone_val = tz;
                    }
                    if let Some(lat) = location.latitude {
                        lat_val = lat;
                    }
                    if let Some(lon) = location.longitude {
                        lon_val = lon;
                    }
                }
            }
        }

        let continent = continent_val.to_string();
        let country_iso = country_iso_val.to_string();
        let country_full = country_full_val.to_string();
        let city = city_val.to_string();
        let timezone = timezone_val.to_string();
        let latitude = if lat_val == 0.0 {
            "0.0".to_string()
        } else {
            lat_val.to_string()
        };
        let longitude = if lon_val == 0.0 {
            "0.0".to_string()
        } else {
            lon_val.to_string()
        };

        // Compile and render the template
        let tmpl = Template::compile(template)
            .map_err(|e| anyhow::anyhow!("Template compilation error: {}", e))?;

        let result = tmpl.render(|field| match field {
            "ip" => ip_str,
            "asnnum" => &asnnum,
            "asnorg" => &asnorg,
            "city" => &city,
            "continent" => &continent,
            "country_iso" => &country_iso,
            "country_full" => &country_full,
            "latitude" => &latitude,
            "longitude" => &longitude,
            "timezone" => &timezone,
            _ => "",
        });

        // Replace spaces with underscores for better terminal display
        Ok(result.replace(' ', "_"))
    }

    fn has_asn(&self, ip_str: &str) -> bool {
        if !self.initialized {
            return false;
        }

        // Parse the IP address
        let ip: IpAddr = match ip_str.parse() {
            Ok(ip) => ip,
            Err(_) => return false,
        };

        // Check if ASN info is available
        if let Some(ref asn_reader) = self.asn_reader {
            if let Ok(asn_record) = asn_reader.lookup::<geoip2::Asn>(ip) {
                return asn_record.autonomous_system_number.is_some();
            }
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
    pub fn lookup(&self, ip: &str, template: &str) -> Result<String> {
        self.get_active_provider()?.lookup(ip, template)
    }

    /// Check if an IP has a valid ASN entry using the active provider
    pub fn has_asn(&self, ip: &str) -> bool {
        if let Ok(provider) = self.get_active_provider() {
            provider.has_asn(ip)
        } else {
            false
        }
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
