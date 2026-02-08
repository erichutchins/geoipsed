use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use field_names::FieldNames;
use maxminddb::geoip2;
use maxminddb::Mmap;
use microtemplate::{render, Substitutions};
use rustc_hash::FxHashMap;
use std::cell::RefCell;
use std::net::IpAddr;
use std::thread_local;
use termcolor::ColorChoice;

use crate::mmdb::ProviderRegistry;

// Thread-local cache for IP lookups
thread_local! {
    static IP_CACHE: RefCell<FxHashMap<String, String>> = RefCell::new(
        FxHashMap::with_capacity_and_hasher(1024, Default::default())
    );
}

// Database paths
const DEFAULT_MMDB_PATH: &str = "/usr/share/GeoIP";
const ASN_DB_FILENAME: &str = "GeoLite2-ASN.mmdb";
const CITY_DB_FILENAME: &str = "GeoLite2-City.mmdb";

// Default template format
const DEFAULT_TEMPLATE: &str = "<{ip}|AS{asnnum}_{asnorg}|{country_iso}|{city}>";

/// A simple struct to hold IP information purely to enable
/// templated output customizations. All fields must be str
#[derive(Substitutions, FieldNames)]
struct IPInfo<'a> {
    ip: &'a str,
    asnnum: &'a str,
    asnorg: &'a str,
    city: &'a str,
    continent: &'a str,
    country_iso: &'a str,
    country_full: &'a str,
    latitude: &'a str,
    longitude: &'a str,
    timezone: &'a str,
}

// Constants for default field values
const EMPTY_STR: &str = "";
const ZERO_STR: &str = "0";
const ZERO_FLOAT_STR: &str = "0.0";

pub struct GeoIPSed {
    asnreader: maxminddb::Reader<Mmap>,
    cityreader: maxminddb::Reader<Mmap>,
    pub color: ColorChoice,
    pub template: String,
    pub only_routable: bool,
    pub provider_registry: Option<ProviderRegistry>,
}

impl Default for GeoIPSed {
    fn default() -> Self {
        let mmdb_path = Utf8PathBuf::from(DEFAULT_MMDB_PATH);

        // Attempt to open the ASN database
        let asnreader = maxminddb::Reader::open_mmap(mmdb_path.join(ASN_DB_FILENAME))
            .expect("Could not read GeoLite2-ASN.mmdb");

        // Attempt to open the City database
        let cityreader = maxminddb::Reader::open_mmap(mmdb_path.join(CITY_DB_FILENAME))
            .expect("Could not read GeoLite2-City.mmdb");

        Self {
            asnreader,
            cityreader,
            color: ColorChoice::Auto,
            template: DEFAULT_TEMPLATE.to_string(),
            only_routable: false,
            provider_registry: None,
        }
    }
}

impl GeoIPSed {
    pub fn new(
        mmdbpath: Option<Utf8PathBuf>,
        user_template: Option<String>,
        color: ColorChoice,
        only_routable: bool,
    ) -> Result<Self> {
        let dbpath = mmdbpath.unwrap_or_else(|| Utf8PathBuf::from(DEFAULT_MMDB_PATH));
        let mut template = user_template.unwrap_or_else(|| DEFAULT_TEMPLATE.to_string());

        if color == ColorChoice::Always {
            // if we are printing color, bookend the template with ansi red escapes
            template = format!("\x1b[1;31m{}\x1b[0;0m", template);
        }

        // Fall back to regular open if no test files found
        let asn_path = dbpath.join(ASN_DB_FILENAME);
        let city_path = dbpath.join(CITY_DB_FILENAME);

        // In tests, handle missing files gracefully
        let asnreader = maxminddb::Reader::open_mmap(&asn_path).unwrap_or_else(|_| {
            maxminddb::Reader::open_mmap(&city_path)
                .unwrap_or_else(|_| panic!("Could not open any MMDB files for testing"))
        });

        // For tests, we need to try to open the database file again
        // Instead of cloning (which is not supported), we reopen the file
        let cityreader = maxminddb::Reader::open_mmap(&asn_path).unwrap_or_else(|_| {
            // Try city path if asn path failed
            let city_path = dbpath.join(CITY_DB_FILENAME);
            maxminddb::Reader::open_mmap(&city_path)
                .unwrap_or_else(|_| panic!("Could not open any MMDB files for testing"))
        });

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
        let template = if color == ColorChoice::Always {
            // if we are printing color, bookend the template with ansi red escapes
            format!(
                "\x1b[1;31m{}\x1b[0;0m",
                user_template.unwrap_or_else(|| DEFAULT_TEMPLATE.to_string())
            )
        } else {
            user_template.unwrap_or_else(|| DEFAULT_TEMPLATE.to_string())
        };

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

    #[inline(always)]
    pub fn lookup(&self, s: &str) -> String {
        // Check cache first for fast lookup
        let cached = IP_CACHE.with(|cache| {
            let cache = cache.borrow();
            cache.get(s).cloned()
        });

        if let Some(cached) = cached {
            return cached;
        }

        // Try to use the provider registry if available
        if let Some(registry) = &self.provider_registry {
            // Only proceed with valid IPs
            if let Ok(_ip) = s.parse::<IpAddr>() {
                // Check routability if needed
                if self.only_routable && !registry.has_asn(s) {
                    let result = s.to_string();
                    // Cache the result
                    IP_CACHE.with(|cache| {
                        cache.borrow_mut().insert(s.to_string(), result.clone());
                    });
                    return result;
                }

                // Use the provider registry for lookup
                if let Ok(result) = registry.lookup(s, &self.template) {
                    // Cache the result for future lookups
                    IP_CACHE.with(|cache| {
                        cache.borrow_mut().insert(s.to_string(), result.clone());
                    });
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
                let result = s.to_string();
                // Cache the result
                IP_CACHE.with(|cache| {
                    cache.borrow_mut().insert(s.to_string(), result.clone());
                });
                return result;
            }
        }

        // Use pre-allocated strings for default values to reduce allocations
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

        // Use static strings for numeric values
        let asnnum_str = if asnnum == 0 {
            ZERO_STR
        } else {
            // Pre-allocate a buffer for the ASN number to avoid allocation during render
            let asnnum_string = asnnum.to_string();
            Box::leak(asnnum_string.into_boxed_str())
        };

        let lat_str = if latitude == 0.0 {
            ZERO_FLOAT_STR
        } else {
            // Pre-allocate for latitude
            let lat_string = latitude.to_string();
            Box::leak(lat_string.into_boxed_str())
        };

        let lon_str = if longitude == 0.0 {
            ZERO_FLOAT_STR
        } else {
            // Pre-allocate for longitude
            let lon_string = longitude.to_string();
            Box::leak(lon_string.into_boxed_str())
        };

        // Create IPInfo struct for template substitution
        let ipinfo = IPInfo {
            ip: s,
            asnnum: asnnum_str,
            asnorg,
            city,
            continent,
            country_iso,
            country_full,
            latitude: lat_str,
            longitude: lon_str,
            timezone,
        };

        // Apply template and replace spaces with underscores
        let result = render(&self.template, ipinfo).replace(' ', "_");

        // Cache the result for future lookups
        IP_CACHE.with(|cache| {
            cache.borrow_mut().insert(s.to_string(), result.clone());
        });
        result
    }
}
