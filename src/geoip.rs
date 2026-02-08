use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use maxminddb::geoip2;
use maxminddb::Mmap;
use std::net::IpAddr;
use termcolor::ColorChoice;

use crate::mmdb::ProviderRegistry;
use crate::template::Template;

// Database paths
const DEFAULT_MMDB_PATH: &str = "/usr/share/GeoIP";
const ASN_DB_FILENAME: &str = "GeoLite2-ASN.mmdb";
const CITY_DB_FILENAME: &str = "GeoLite2-City.mmdb";

// Default template format
const DEFAULT_TEMPLATE: &str = "<{ip}|AS{asnnum}_{asnorg}|{country_iso}|{city}>";

// Constants for default field values
const EMPTY_STR: &str = "";
const ZERO_STR: &str = "0";
const ZERO_FLOAT_STR: &str = "0.0";

pub struct GeoIPSed {
    asnreader: maxminddb::Reader<Mmap>,
    cityreader: maxminddb::Reader<Mmap>,
    pub color: ColorChoice,
    template: Template,
    pub only_routable: bool,
    pub provider_registry: Option<ProviderRegistry>,
}

impl GeoIPSed {
    pub fn new(
        mmdbpath: Option<Utf8PathBuf>,
        user_template: Option<String>,
        color: ColorChoice,
        only_routable: bool,
    ) -> Result<Self> {
        let dbpath = mmdbpath.unwrap_or_else(|| Utf8PathBuf::from(DEFAULT_MMDB_PATH));
        let mut template_str = user_template.unwrap_or_else(|| DEFAULT_TEMPLATE.to_string());

        if color == ColorChoice::Always {
            // if we are printing color, bookend the template with ansi red escapes
            template_str = format!("\x1b[1;31m{}\x1b[0;0m", template_str);
        }

        // Compile the template once during initialization
        let template = Template::compile(&template_str)
            .map_err(|e| anyhow::anyhow!("Invalid template: {}", e))?;

        // Open database files
        let asn_path = dbpath.join(ASN_DB_FILENAME);
        let city_path = dbpath.join(CITY_DB_FILENAME);

        // Open ASN database
        let asnreader = maxminddb::Reader::open_mmap(&asn_path)
            .with_context(|| format!("Failed to open ASN database at {}", asn_path))?;

        // Open City database
        let cityreader = maxminddb::Reader::open_mmap(&city_path)
            .with_context(|| format!("Failed to open City database at {}", city_path))?;

        Ok(Self {
            asnreader,
            cityreader,
            color,
            template,
            only_routable,
            provider_registry: None,
        })
    }

    #[inline]
    pub fn new_with_provider(
        mmdbpath: Option<Utf8PathBuf>,
        user_template: Option<String>,
        color: ColorChoice,
        only_routable: bool,
        provider_registry: ProviderRegistry,
    ) -> Result<Self> {
        let template_str = if color == ColorChoice::Always {
            // if we are printing color, bookend the template with ansi red escapes
            format!(
                "\x1b[1;31m{}\x1b[0;0m",
                user_template.unwrap_or_else(|| DEFAULT_TEMPLATE.to_string())
            )
        } else {
            user_template.unwrap_or_else(|| DEFAULT_TEMPLATE.to_string())
        };

        // Compile the template once during initialization
        let template = Template::compile(&template_str)
            .map_err(|e| anyhow::anyhow!("Invalid template: {}", e))?;

        // For backwards compatibility, we still need the original readers
        let dbpath = mmdbpath
            .clone()
            .unwrap_or_else(|| Utf8PathBuf::from(DEFAULT_MMDB_PATH));

        // Open ASN database with error context
        let asn_path = dbpath.join(ASN_DB_FILENAME);
        let asnreader = maxminddb::Reader::open_mmap(&asn_path)
            .with_context(|| format!("Failed to open ASN database at {}", asn_path))?;

        // Open City database with error context
        let city_path = dbpath.join(CITY_DB_FILENAME);
        let cityreader = maxminddb::Reader::open_mmap(&city_path)
            .with_context(|| format!("Failed to open City database at {}", city_path))?;

        Ok(Self {
            asnreader,
            cityreader,
            color,
            template,
            only_routable,
            provider_registry: Some(provider_registry),
        })
    }

    #[inline]
    pub fn lookup(&self, s: &str) -> String {
        // Try to use the provider registry if available
        if let Some(registry) = &self.provider_registry {
            // Only proceed with valid IPs
            if let Ok(_ip) = s.parse::<IpAddr>() {
                // Check routability if needed
                if self.only_routable && !registry.has_asn(s) {
                    return s.to_string();
                }

                // Use the provider registry for lookup
                if let Ok(result) = registry.lookup(s, &self.template.to_string()) {
                    return result;
                }
            }

            // Fall back to the original method if provider lookup fails
        }

        // Legacy lookup method
        let ip: IpAddr = match s.parse() {
            Ok(ip) => ip,
            // if not a valid IP address, just return it unchanged
            Err(_) => return s.to_string(),
        };

        // Check if we need to validate for routability
        if self.only_routable {
            // Try to fetch ASN info to check if routable - just do a quick lookup
            if self.asnreader.lookup::<geoip2::Asn>(ip).is_err() {
                return s.to_string();
            }
        }

        // Initialize all fields with default values
        let mut asnnum: u32 = 0;
        let mut asnorg: &str = EMPTY_STR;
        let mut city: &str = EMPTY_STR;
        let mut continent: &str = EMPTY_STR;
        let mut country_iso: &str = EMPTY_STR;
        let mut country_full: &str = EMPTY_STR;
        let mut latitude: f64 = 0.0;
        let mut longitude: f64 = 0.0;
        let mut timezone: &str = EMPTY_STR;

        // Look up ASN information
        if let Ok(asnrecord) = self.asnreader.lookup::<geoip2::Asn>(ip) {
            asnnum = asnrecord.autonomous_system_number.unwrap_or(0);
            asnorg = asnrecord
                .autonomous_system_organization
                .unwrap_or(EMPTY_STR);
        }

        // Look up city/country information - optimize for most common paths
        if let Ok(cityrecord) = self.cityreader.lookup::<geoip2::City>(ip) {
            // Extract continent information
            continent = cityrecord
                .continent
                .and_then(|c| c.code)
                .unwrap_or(EMPTY_STR);

            // Extract country information
            if let Some(c) = cityrecord.country {
                country_iso = c.iso_code.unwrap_or(EMPTY_STR);
                if let Some(n) = c.names {
                    country_full = n.get("en").unwrap_or(&EMPTY_STR);
                }
            }

            // get city name, hard coded for en language currently
            city = match cityrecord.city.and_then(|c| c.names) {
                Some(names) => names.get("en").unwrap_or(&EMPTY_STR),
                None => EMPTY_STR,
            };

            // Extract location information
            if let Some(locrecord) = cityrecord.location {
                timezone = locrecord.time_zone.unwrap_or(EMPTY_STR);
                latitude = locrecord.latitude.unwrap_or(0.0);
                longitude = locrecord.longitude.unwrap_or(0.0);
            };
        }

        // Convert numeric fields to strings
        let asnnum_str = if asnnum == 0 {
            ZERO_STR
        } else {
            &asnnum.to_string()
        };

        let lat_str = if latitude == 0.0 {
            ZERO_FLOAT_STR
        } else {
            &latitude.to_string()
        };

        let lon_str = if longitude == 0.0 {
            ZERO_FLOAT_STR
        } else {
            &longitude.to_string()
        };

        // Render the template using closure-based lookup
        let result = self.template.render(|field| match field {
            "ip" => s,
            "asnnum" => asnnum_str,
            "asnorg" => asnorg,
            "city" => city,
            "continent" => continent,
            "country_iso" => country_iso,
            "country_full" => country_full,
            "latitude" => lat_str,
            "longitude" => lon_str,
            "timezone" => timezone,
            _ => EMPTY_STR,
        });

        // Replace spaces with underscores
        result.replace(' ', "_")
    }
}
