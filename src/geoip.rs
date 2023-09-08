use camino::Utf8PathBuf;
use field_names::FieldNames;
use maxminddb::geoip2;
use maxminddb::Mmap;
use microtemplate::{render, Substitutions};
use std::net::IpAddr;
use termcolor::ColorChoice;

// ipv4 - copied from cyberchef.org minus the cidr mask
// ipv6 - https://gist.github.com/dfee/6ed3a4b05cfe7a6faf40a2102408d5d8
// note that rust regex does not support look around parameters
pub const REGEX_PATTERN: &str = r"(?x)
    (
        (?:(?:\d|[01]?\d\d|2[0-4]\d|25[0-5])\.){3}(?:25[0-5]|2[0-4]\d|[01]?\d\d|\d)
    )
    |
    (
        (?:(?:(?:(?:[0-9a-fA-F]){1,4}):){1,4}:[^\s:](?:(?:(?:25[0-5]|(?:2[0-4]|1{0,1}[0-9]){0,1}[0-9]).){3,3}(?:25[0-5]|(?:2[0-4]|1{0,1}[0-9]){0,1}[0-9])))|(?:::(?:ffff(?::0{1,4}){0,1}:){0,1}[^\s:](?:(?:(?:25[0-5]|(?:2[0-4]|1{0,1}[0-9]){0,1}[0-9]).){3,3}(?:25[0-5]|(?:2[0-4]|1{0,1}[0-9]){0,1}[0-9])))|(?:fe80:(?::(?:(?:[0-9a-fA-F]){1,4})){0,4}%[0-9a-zA-Z]{1,})|(?::(?:(?::(?:(?:[0-9a-fA-F]){1,4})){1,7}|:))|(?:(?:(?:[0-9a-fA-F]){1,4}):(?:(?::(?:(?:[0-9a-fA-F]){1,4})){1,6}))|(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){1,2}(?::(?:(?:[0-9a-fA-F]){1,4})){1,5})|(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){1,3}(?::(?:(?:[0-9a-fA-F]){1,4})){1,4})|(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){1,4}(?::(?:(?:[0-9a-fA-F]){1,4})){1,3})|(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){1,5}(?::(?:(?:[0-9a-fA-F]){1,4})){1,2})|(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){1,6}:(?:(?:[0-9a-fA-F]){1,4}))|(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){1,7}:)|(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){7,7}(?:(?:[0-9a-fA-F]){1,4}))
    )";

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

pub fn print_ip_field_names() {
    println!("Available template geoip field names are:");
    for f in IPInfo::FIELDS {
        println!("{{{f}}}");
    }
}

pub struct GeoIPSed {
    asnreader: maxminddb::Reader<Mmap>,
    cityreader: maxminddb::Reader<Mmap>,
    pub color: ColorChoice,
    pub template: String,
}

impl Default for GeoIPSed {
    fn default() -> Self {
        Self {
            asnreader: maxminddb::Reader::open_mmap("/usr/share/GeoIP/GeoLite2-ASN.mmdb")
                .expect("Could not read GeoLite2-ASN.mmdb"),
            cityreader: maxminddb::Reader::open_mmap("/usr/share/GeoIP/GeoLite2-City.mmdb")
                .expect("Could not read GeoLite2-City.mmdb"),
            color: ColorChoice::Auto,
            template: "<{ip}|AS{asnnum}_{asnorg}|{country_iso}|{city}>".to_string(),
        }
    }
}

impl GeoIPSed {
    pub fn new(
        mmdbpath: Option<Utf8PathBuf>,
        user_template: Option<String>,
        color: ColorChoice,
    ) -> Self {
        let dbpath = mmdbpath.unwrap_or_else(|| Utf8PathBuf::from("/usr/share/GeoIP"));
        let mut template = user_template
            .unwrap_or_else(|| "<{ip}|AS{asnnum}_{asnorg}|{country_iso}|{city}>".to_string());

        if color == ColorChoice::Always {
            // if we are printing color, bookend the template with ansi red escapes
            template = format!("\x1b[1;31m{}\x1b[0;0m", template);
        }

        Self {
            asnreader: maxminddb::Reader::open_mmap(dbpath.join("GeoLite2-ASN.mmdb"))
                .expect("Could not read GeoLite2-ASN.mmdb"),
            cityreader: maxminddb::Reader::open_mmap(dbpath.join("GeoLite2-City.mmdb"))
                .expect("Could not read GeoLite2-City.mmdb"),
            color,
            template,
        }
    }

    #[inline]
    pub fn lookup(&self, s: &str) -> String {
        let ip: IpAddr = match s.parse() {
            Ok(ip) => ip,
            // if not an ip, just return and be done
            Err(_) => return s.to_string(),
        };

        // if match ip {
        //     IpAddr::V4(ip) => {
        //         ip.is_loopback() || ip.is_private() || ip.is_link_local() || ip.is_broadcast()
        //     }
        //     IpAddr::V6(ip) => ip.is_loopback(),
        // } {
        //     return format!("{}|||", s);
        // }

        let mut asnnum: u32 = 0;
        let mut asnorg: &str = "";
        let mut city: &str = "";
        let mut continent: &str = "";
        let mut country_iso: &str = "";
        let mut country_full: &str = "";
        let mut latitude: f64 = 0.0;
        let mut longitude: f64 = 0.0;
        let mut timezone: &str = "";

        if let Ok(asnrecord) = self.asnreader.lookup::<geoip2::Asn>(ip) {
            asnnum = asnrecord.autonomous_system_number.unwrap_or(0);
            asnorg = asnrecord.autonomous_system_organization.unwrap_or("");
        };

        if let Ok(cityrecord) = self.cityreader.lookup::<geoip2::City>(ip) {
            // from https://github.com/oschwald/maxminddb-rust/blob/main/examples/within.rs
            continent = cityrecord.continent.and_then(|c| c.code).unwrap_or("");
            if let Some(c) = cityrecord.country {
                country_iso = c.iso_code.unwrap_or("");
                if let Some(n) = c.names {
                    country_full = n.get("en").unwrap_or(&"");
                }
            }

            // get city name, hard coded for en language currently
            city = match cityrecord.city.and_then(|c| c.names) {
                Some(names) => names.get("en").unwrap_or(&""),
                None => "",
            };

            // pull out location specific fields
            if let Some(locrecord) = cityrecord.location {
                timezone = locrecord.time_zone.unwrap_or("");
                latitude = locrecord.latitude.unwrap_or(0.0);
                longitude = locrecord.longitude.unwrap_or(0.0);
            };
        };

        // create ipinfo struct just for purposes of applying template
        let ipinfo = IPInfo {
            ip: s,
            asnnum: &asnnum.to_string(),
            asnorg,
            city,
            continent,
            country_iso,
            country_full,
            latitude: &latitude.to_string(),
            longitude: &longitude.to_string(),
            timezone,
        };

        // apply template to render enrichment per user-specification
        render(&self.template, ipinfo).replace(' ', "_")
    }
}
