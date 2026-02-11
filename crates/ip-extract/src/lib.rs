//! High-performance IP address extraction and tagging engine.
//!
//! This crate provides the `Extractor` for finding IPv4 and IPv6 addresses in raw bytes,
//! and the `Tagged` system for generating decorated output or JSON metadata.
//!
//! # Performance Architecture
//!
//! This crate uses `regex-automata` with HIR (High-level Intermediate Representation) for
//! optimal performance:
//!
//! - **Pre-compiled patterns**: Regex patterns are compiled once via `OnceLock` and reused
//!   across all extractor instances, eliminating runtime compilation overhead.
//! - **HIR compilation**: Patterns are parsed into HIR first, then compiled to automata.
//!   This allows inspection and manipulation of regex structure before compilation.
//! - **Multi-pattern matching**: The combined IPv4+IPv6 regex uses `build_many_from_hir`,
//!   which enables simultaneous matching of multiple patterns in a single pass with pattern
//!   ID tracking for efficient validation routing.
//! - **Cheap cloning**: `regex-automata::meta::Regex` clones share read-only automata data,
//!   making it efficient to create per-thread extractors without memory overhead.
//!
//! # Examples
//!
//! ```
//! use ip_extract::ExtractorBuilder;
//!
//! let extractor = ExtractorBuilder::new()
//!     .ipv4(true)
//!     .private_ips(true)
//!     .build()
//!     .unwrap();
//!
//! let haystack = b"Local: 192.168.1.1, Remote: 8.8.8.8";
//! let matches: Vec<_> = extractor.find_iter(haystack).collect();
//! assert_eq!(matches.len(), 2);
//! ```

use std::borrow::Cow;
use std::net::{IpAddr, Ipv4Addr};
use std::ops::Range;
use std::str;
use std::sync::OnceLock;

use regex_automata::meta::Regex;

mod tag;
pub use tag::{Tag, Tagged, TextData};

// IP pattern strings
static IPV4_PATTERN: &str =
    r"(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)";
static IPV6_PATTERN: &str = r"(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){1,4}:[^\s:](?:(?:(?:25[0-5]|(?:2[0-4]|1{0,1}[0-9]){0,1}[0-9]).){3,3}(?:25[0-5]|(?:2[0-4]|1{0,1}[0-9]){0,1}[0-9])))|(?:::(?:ffff(?::0{1,4}){0,1}:){0,1}[^\s:](?:(?:(?:25[0-5]|(?:2[0-4]|1{0,1}[0-9]){0,1}[0-9]).){3,3}(?:25[0-5]|(?:2[0-4]|1{0,1}[0-9]){0,1}[0-9])))|(?:fe80:(?::(?:(?:[0-9a-fA-F]){1,4})){0,4}%[0-9a-zA-Z]{1,})|(?::(?:(?::(?:(?:[0-9a-fA-F]){1,4})){1,7}|:))|(?:(?:(?:[0-9a-fA-F]){1,4}):(?:(?::(?:(?:[0-9a-fA-F]){1,4})){1,6}))|(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){1,2}(?::(?:(?:[0-9a-fA-F]){1,4})){1,5})|(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){1,3}(?::(?:(?:[0-9a-fA-F]){1,4})){1,4})|(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){1,4}(?::(?:(?:[0-9a-fA-F]){1,4})){1,3})|(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){1,5}(?::(?:(?:[0-9a-fA-F]){1,4})){1,2})|(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){1,6}:(?:(?:[0-9a-fA-F]){1,4}))|(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){1,7}:)|(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){7,7}(?:(?:[0-9a-fA-F]){1,4}))";

// Pre-compiled regex variants (lazy-initialized once on first use)
static REGEX_IPV4_ONLY: OnceLock<Regex> = OnceLock::new();
static REGEX_IPV6_ONLY: OnceLock<Regex> = OnceLock::new();
static REGEX_BOTH: OnceLock<Regex> = OnceLock::new();

/// Get the pre-compiled IPv4-only regex pattern.
///
/// This function uses `OnceLock` to lazily compile the IPv4 pattern on first use.
/// The pattern is parsed into HIR (High-level Intermediate Representation) before
/// compilation, which provides:
///
/// - **Structural manipulation**: HIR can be inspected and modified before compilation
/// - **Optimization opportunities**: The compiler can apply HIR-level optimizations
/// - **Multi-pattern support**: HIR enables efficient combined pattern matching
///
/// The compiled regex uses `auto_prefilter(true)` to enable fast literal scanning
/// and `LeftmostFirst` matching semantics for consistent extraction behavior.
fn get_ipv4_regex() -> &'static Regex {
    REGEX_IPV4_ONLY.get_or_init(|| {
        let ipv4_hir = regex_syntax::Parser::new()
            .parse(IPV4_PATTERN)
            .expect("IPv4 pattern should be valid");

        Regex::builder()
            .configure(
                Regex::config()
                    .auto_prefilter(true)
                    .match_kind(regex_automata::MatchKind::LeftmostFirst),
            )
            .build_from_hir(&ipv4_hir)
            .expect("IPv4 regex should compile")
    })
}

/// Get the pre-compiled IPv6-only regex pattern.
fn get_ipv6_regex() -> &'static Regex {
    REGEX_IPV6_ONLY.get_or_init(|| {
        let ipv6_hir = regex_syntax::Parser::new()
            .parse(IPV6_PATTERN)
            .expect("IPv6 pattern should be valid");

        Regex::builder()
            .configure(
                Regex::config()
                    .auto_prefilter(true)
                    .match_kind(regex_automata::MatchKind::LeftmostFirst),
            )
            .build_from_hir(&ipv6_hir)
            .expect("IPv6 regex should compile")
    })
}

/// Get the pre-compiled combined IPv4+IPv6 regex pattern.
///
/// This function compiles both IPv4 and IPv6 patterns into a single multi-pattern regex
/// using `build_many_from_hir`. The HIR-based approach is essential here because:
///
/// - **Pattern ID tracking**: Each match includes a pattern ID (0=IPv4, 1=IPv6) that
///   routes to the correct validator without string inspection.
/// - **Single-pass matching**: Both patterns are matched simultaneously in one automaton
///   traversal, rather than running two separate regex engines.
/// - **Shared prefix optimization**: The automaton can share common states between patterns
///   for more efficient matching.
///
/// This is significantly more efficient than alternation (`ipv4|ipv6`) or sequential matching.
fn get_both_regex() -> &'static Regex {
    REGEX_BOTH.get_or_init(|| {
        let ipv4_hir = regex_syntax::Parser::new()
            .parse(IPV4_PATTERN)
            .expect("IPv4 pattern should be valid");
        let ipv6_hir = regex_syntax::Parser::new()
            .parse(IPV6_PATTERN)
            .expect("IPv6 pattern should be valid");

        Regex::builder()
            .configure(
                Regex::config()
                    .auto_prefilter(true)
                    .match_kind(regex_automata::MatchKind::LeftmostFirst),
            )
            .build_many_from_hir(&[Cow::Borrowed(&ipv4_hir), Cow::Borrowed(&ipv6_hir)])
            .expect("Combined regex should compile")
    })
}

/// Internal validator types for different IP versions.
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
                    parse_ipv4_bytes(bytes).is_some()
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
}

impl Extractor {
    /// Returns an iterator over all valid IP address matches in the given haystack.
    ///
    /// The iterator yields `Range<usize>` objects representing the start and end
    /// positions of each match in the `haystack`. Matches are found from left-to-right.
    /// Only matches that pass the configured validation rules are returned.
    #[inline]
    pub fn find_iter<'a>(&'a self, haystack: &'a [u8]) -> impl Iterator<Item = Range<usize>> + 'a {
        self.regex.find_iter(haystack).filter_map(move |m| {
            let pid = m.pattern();
            let range = m.range();

            // Validate the match using the corresponding validator
            if !self.validators[pid.as_usize()].validate(&haystack[range.clone()]) {
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
    /// Create a new builder with default settings.
    ///
    /// By default:
    /// - IPv4 is enabled.
    /// - IPv6 is enabled.
    /// - Private, Loopback, and Broadcast IPs are **excluded**.
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

    /// Set whether to include IPv4 addresses in the search.
    #[inline]
    pub fn ipv4(&mut self, include: bool) -> &mut Self {
        self.include_ipv4 = include;
        self
    }

    /// Set whether to include IPv6 addresses in the search.
    #[inline]
    pub fn ipv6(&mut self, include: bool) -> &mut Self {
        self.include_ipv6 = include;
        self
    }

    /// Set whether to include private (RFC 1918) IP addresses.
    #[inline]
    pub fn private_ips(&mut self, include: bool) -> &mut Self {
        self.include_private = include;
        self
    }

    /// Set whether to include loopback (127.0.0.1/::1) IP addresses.
    #[inline]
    pub fn loopback_ips(&mut self, include: bool) -> &mut Self {
        self.include_loopback = include;
        self
    }

    /// Set whether to include broadcast and link-local IP addresses.
    #[inline]
    pub fn broadcast_ips(&mut self, include: bool) -> &mut Self {
        self.include_broadcast = include;
        self
    }

    /// Set whether to only include internet-routable IP addresses.
    ///
    /// Note: Actual routability check usually requires external metadata (like ASN database),
    /// so this flag is primarily used as a hint for higher-level filters.
    #[inline]
    pub fn only_routable(&mut self, only: bool) -> &mut Self {
        self.only_routable = only;
        self
    }

    /// Consumes the builder and returns a compiled `Extractor`.
    ///
    /// This uses pre-compiled regex patterns for optimal performance. Returns an error if:
    /// - No IP versions are enabled.
    pub fn build(&self) -> anyhow::Result<Extractor> {
        let (regex, validators) = match (self.include_ipv4, self.include_ipv6) {
            (true, true) => {
                let validators = vec![
                    ValidatorType::IPv4 {
                        include_private: self.include_private,
                        include_loopback: self.include_loopback,
                        include_broadcast: self.include_broadcast,
                        only_routable: self.only_routable,
                    },
                    ValidatorType::IPv6 {
                        include_private: self.include_private,
                        include_loopback: self.include_loopback,
                        only_routable: self.only_routable,
                    },
                ];
                (get_both_regex().clone(), validators)
            }
            (true, false) => {
                let validators = vec![ValidatorType::IPv4 {
                    include_private: self.include_private,
                    include_loopback: self.include_loopback,
                    include_broadcast: self.include_broadcast,
                    only_routable: self.only_routable,
                }];
                (get_ipv4_regex().clone(), validators)
            }
            (false, true) => {
                let validators = vec![ValidatorType::IPv6 {
                    include_private: self.include_private,
                    include_loopback: self.include_loopback,
                    only_routable: self.only_routable,
                }];
                (get_ipv6_regex().clone(), validators)
            }
            (false, false) => {
                anyhow::bail!("No IP address patterns selected");
            }
        };

        Ok(Extractor { regex, validators })
    }
}

/// Validate an IPv4 address
fn validate_ipv4(
    bytes: &[u8],
    include_private: bool,
    include_loopback: bool,
    include_broadcast: bool,
    _only_routable: bool,
) -> bool {
    // Parse the IP address directly from bytes
    let ipv4 = match parse_ipv4_bytes(bytes) {
        Some(ip) => ip,
        None => return false,
    };

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

/// Parse an IPv4 address from a byte slice without UTF-8 conversion.
/// This strictly matches the format [0-255].[0-255].[0-255].[0-255]
/// and disallows leading zeros in multi-digit octets (matching std::net::Ipv4Addr).
#[inline]
pub fn parse_ipv4_bytes(bytes: &[u8]) -> Option<Ipv4Addr> {
    if bytes.len() < 7 || bytes.len() > 15 {
        return None;
    }

    let mut octets = [0u8; 4];
    let mut octet_idx = 0;
    let mut current_val = 0u16;
    let mut digits_in_octet = 0;

    for &b in bytes {
        if b == b'.' {
            if digits_in_octet == 0 || octet_idx == 3 {
                return None;
            }
            octets[octet_idx] = current_val as u8;
            octet_idx += 1;
            current_val = 0;
            digits_in_octet = 0;
        } else if b.is_ascii_digit() {
            let digit = (b - b'0') as u16;
            // Check for leading zero
            if digits_in_octet > 0 && current_val == 0 {
                return None;
            }
            current_val = current_val * 10 + digit;
            if current_val > 255 {
                return None;
            }
            digits_in_octet += 1;
        } else {
            return None;
        }
    }

    if octet_idx != 3 || digits_in_octet == 0 {
        return None;
    }
    octets[3] = current_val as u8;

    Some(Ipv4Addr::new(octets[0], octets[1], octets[2], octets[3]))
}

/// Validate an IPv6 address
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
