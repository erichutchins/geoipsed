use std::borrow::Cow;
use std::net::IpAddr;
use std::ops::Range;
use std::str;

use regex_automata::meta::Regex;
use regex_syntax::hir::Hir;

/// The types of validators we support
#[derive(Clone, Debug)]
enum ValidatorType {
    IPv4 {
        include_private: bool,
        include_loopback: bool,
        include_broadcast: bool,
        only_routable: bool,
    },
    IPv6 {
        include_private: bool,
        include_loopback: bool,
        only_routable: bool,
    },
}

impl ValidatorType {
    #[inline(always)]
    fn validate(&self, bytes: &[u8]) -> bool {
        match *self {
            ValidatorType::IPv4 {
                include_private,
                include_loopback,
                include_broadcast,
                only_routable,
            } => {
                // Fast path for common case (all included)
                if include_private && include_loopback && include_broadcast && !only_routable {
                    // In this case we only need to validate it's a valid IP, which the regex already did
                    let s = match std::str::from_utf8(bytes) {
                        Ok(s) => s,
                        Err(_) => return false,
                    };
                    s.parse::<std::net::Ipv4Addr>().is_ok()
                } else {
                    validate_ipv4(
                        bytes,
                        include_private,
                        include_loopback,
                        include_broadcast,
                        only_routable,
                    )
                }
            }
            ValidatorType::IPv6 {
                include_private,
                include_loopback,
                only_routable,
            } => {
                // Fast path for common case (all included)
                if include_private && include_loopback && !only_routable {
                    // In this case we only need to validate it's a valid IP, which the regex already did
                    let s = match std::str::from_utf8(bytes) {
                        Ok(s) => s,
                        Err(_) => return false,
                    };
                    s.parse::<std::net::Ipv6Addr>().is_ok()
                } else {
                    validate_ipv6(bytes, include_private, include_loopback, only_routable)
                }
            }
        }
    }
}

/// A searcher for finding IPv4 and IPv6 addresses in text.
#[derive(Clone, Debug)]
pub struct Extractor {
    regex: Regex,
    validators: Vec<ValidatorType>,
    pattern_indices: Vec<usize>,
}

impl Extractor {
    /// Return an iterator of IP address matches found in the haystack.
    #[inline(always)]
    pub fn find_iter<'a>(&'a self, haystack: &'a [u8]) -> impl Iterator<Item = Range<usize>> + 'a {
        self.regex.captures_iter(haystack).filter_map(move |caps| {
            let pid = caps.pattern()?;
            // Use the appropriate capture group based on the pattern index
            let span = caps.get_group(self.pattern_indices[pid])?;
            let range = span.range();

            // Validate the match using the corresponding validator
            if !self.validators[pid].validate(&haystack[range.clone()]) {
                return None;
            }

            Some(range)
        })
    }
}

/// Builder for constructing an IP address extractor with custom settings.
#[derive(Clone, Debug, Default)]
pub struct ExtractorBuilder {
    include_ipv4: bool,
    include_ipv6: bool,
    include_private: bool,
    include_loopback: bool,
    include_broadcast: bool,
    only_routable: bool,
}

impl ExtractorBuilder {
    /// Create a new builder with default settings (both IPv4 and IPv6, exclude special IPs).
    #[inline]
    pub fn new() -> Self {
        Self {
            include_ipv4: true,
            include_ipv6: true,
            include_private: false,
            include_loopback: false,
            include_broadcast: false,
            only_routable: false,
        }
    }

    /// Include or exclude IPv4 addresses.
    #[inline(always)]
    pub fn ipv4(&mut self, include: bool) -> &mut Self {
        self.include_ipv4 = include;
        self
    }

    /// Include or exclude IPv6 addresses.
    #[inline(always)]
    pub fn ipv6(&mut self, include: bool) -> &mut Self {
        self.include_ipv6 = include;
        self
    }

    /// Include or exclude private IP addresses.
    #[inline(always)]
    pub fn private_ips(&mut self, include: bool) -> &mut Self {
        self.include_private = include;
        self
    }

    /// Include or exclude loopback IP addresses.
    #[inline(always)]
    pub fn loopback_ips(&mut self, include: bool) -> &mut Self {
        self.include_loopback = include;
        self
    }

    /// Include or exclude broadcast IP addresses.
    #[inline(always)]
    pub fn broadcast_ips(&mut self, include: bool) -> &mut Self {
        self.include_broadcast = include;
        self
    }

    /// Only include internet-routable IP addresses (ones with valid ASN entries).
    #[inline(always)]
    pub fn only_routable(&mut self, only: bool) -> &mut Self {
        self.only_routable = only;
        self
    }

    /// Build the extractor with the current settings.
    pub fn build(&self) -> anyhow::Result<Extractor> {
        // Pre-allocate vectors with known capacity for better performance
        let pattern_count = self.include_ipv4 as usize + self.include_ipv6 as usize;
        let mut patterns: Vec<Cow<'_, Hir>> = Vec::with_capacity(pattern_count);
        let mut validators: Vec<ValidatorType> = Vec::with_capacity(pattern_count);
        let mut pattern_indices: Vec<usize> = Vec::with_capacity(pattern_count);

        // Add IPv4 pattern if included
        if self.include_ipv4 {
            // Use a more efficient IPv4 pattern
            static IPV4_PATTERN: &str = r"(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)";

            let ipv4_hir: Hir = regex_syntax::Parser::new().parse(IPV4_PATTERN)?;

            patterns.push(Cow::Owned(ipv4_hir));
            validators.push(ValidatorType::IPv4 {
                include_private: self.include_private,
                include_loopback: self.include_loopback,
                include_broadcast: self.include_broadcast,
                only_routable: self.only_routable,
            });
            pattern_indices.push(0);
        }

        // Add IPv6 pattern if included
        if self.include_ipv6 {
            // Use a constant for the IPv6 pattern to allow compiler optimization
            static IPV6_PATTERN: &str = r"(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){1,4}:[^\s:](?:(?:(?:25[0-5]|(?:2[0-4]|1{0,1}[0-9]){0,1}[0-9]).){3,3}(?:25[0-5]|(?:2[0-4]|1{0,1}[0-9]){0,1}[0-9])))|(?:::(?:ffff(?::0{1,4}){0,1}:){0,1}[^\s:](?:(?:(?:25[0-5]|(?:2[0-4]|1{0,1}[0-9]){0,1}[0-9]).){3,3}(?:25[0-5]|(?:2[0-4]|1{0,1}[0-9]){0,1}[0-9])))|(?:fe80:(?::(?:(?:[0-9a-fA-F]){1,4})){0,4}%[0-9a-zA-Z]{1,})|(?::(?:(?::(?:(?:[0-9a-fA-F]){1,4})){1,7}|:))|(?:(?:(?:[0-9a-fA-F]){1,4}):(?:(?::(?:(?:[0-9a-fA-F]){1,4})){1,6}))|(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){1,2}(?::(?:(?:[0-9a-fA-F]){1,4})){1,5})|(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){1,3}(?::(?:(?:[0-9a-fA-F]){1,4})){1,4})|(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){1,4}(?::(?:(?:[0-9a-fA-F]){1,4})){1,3})|(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){1,5}(?::(?:(?:[0-9a-fA-F]){1,4})){1,2})|(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){1,6}:(?:(?:[0-9a-fA-F]){1,4}))|(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){1,7}:)|(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){7,7}(?:(?:[0-9a-fA-F]){1,4}))";

            let ipv6_hir: Hir = regex_syntax::Parser::new().parse(IPV6_PATTERN)?;

            patterns.push(Cow::Owned(ipv6_hir));
            validators.push(ValidatorType::IPv6 {
                include_private: self.include_private,
                include_loopback: self.include_loopback,
                only_routable: self.only_routable,
            });
            pattern_indices.push(0);
        }

        // Fast fail if no patterns selected
        if patterns.is_empty() {
            anyhow::bail!("No IP address patterns selected");
        }

        // Create the regex engine with optimized settings for high performance
        let regex = Regex::builder()
            .configure(
                Regex::config()
                    .auto_prefilter(true) // Enable prefilter for better performance
                    .match_kind(regex_automata::MatchKind::LeftmostFirst),
            ) // Use leftmost-first semantics
            .build_many_from_hir(&patterns)?;

        Ok(Extractor {
            regex,
            validators,
            pattern_indices,
        })
    }
}

/// Validate an IPv4 address
#[inline(always)]
fn validate_ipv4(
    bytes: &[u8],
    include_private: bool,
    include_loopback: bool,
    include_broadcast: bool,
    _only_routable: bool,
) -> bool {
    // Fast path: Check common patterns for IPv4 addresses before parsing
    if bytes.len() < 7 || bytes.len() > 15 {
        return false; // Too short or too long to be a valid IPv4
    }

    // Parse the bytes as a string directly
    let s = match std::str::from_utf8(bytes) {
        Ok(s) => s,
        Err(_) => return false,
    };

    // Parse the IP address
    let ip = match s.parse::<IpAddr>() {
        Ok(ip) => ip,
        Err(_) => return false,
    };

    // Process IPv4 addresses
    match ip {
        IpAddr::V4(ipv4) => {
            // Check if we should include all types - fast path
            if include_private && include_loopback && include_broadcast {
                return true;
            }

            // Short-circuit evaluation to avoid unnecessary checks
            if !include_private && ipv4.is_private() {
                return false;
            }

            if !include_loopback && ipv4.is_loopback() {
                return false;
            }

            if !include_broadcast && (ipv4.is_broadcast() || ipv4.is_link_local()) {
                return false;
            }

            // For "only routable" validation, we'll defer to the GeoIPSed component
            true
        }
        _ => false, // Not an IPv4
    }
}

/// Validate an IPv6 address
#[inline(always)]
fn validate_ipv6(
    bytes: &[u8],
    include_private: bool,
    include_loopback: bool,
    _only_routable: bool,
) -> bool {
    // Fast path: Check for IPv6 patterns
    if bytes.len() < 2 {
        return false; // Too short to be a valid IPv6
    }

    // Parse the bytes as a string directly
    let s = match std::str::from_utf8(bytes) {
        Ok(s) => s,
        Err(_) => return false,
    };

    // Parse the IP address
    let ip = match s.parse::<IpAddr>() {
        Ok(ip) => ip,
        Err(_) => return false,
    };

    // Process IPv6 addresses
    match ip {
        IpAddr::V6(ipv6) => {
            // Check if we should include all types - fast path
            if include_private && include_loopback {
                return true;
            }

            // Short-circuit evaluation to avoid unnecessary checks
            if !include_private && ipv6.is_unicast_link_local() {
                return false;
            }

            if !include_loopback && ipv6.is_loopback() {
                return false;
            }

            // For "only routable" validation, we'll defer to the GeoIPSed component
            true
        }
        _ => false, // Not an IPv6
    }
}
