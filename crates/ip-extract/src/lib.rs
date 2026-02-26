//! High-performance IP address extraction and tagging engine.
//!
//! `ip-extract` provides a blazingly fast, configurable extractor for finding IPv4 and IPv6
//! addresses in unstructured text. It achieves maximum throughput through:
//!
//! - **Compile-time DFA**: IP patterns are converted to dense Forward DFAs during build,
//!   eliminating runtime regex compilation and heap allocation.
//! - **Zero-overhead scanning**: The DFA scans at O(n) with no backtracking; validation
//!   is performed only on candidates.
//! - **Strict validation**: Deep checks eliminate false positives (e.g., `1.2.3.4.5` is rejected).
//!
//! ## Quick Start
//!
//! By default, **all IP addresses are extracted**:
//!
//! ```no_run
//! use ip_extract::ExtractorBuilder;
//!
//! # fn main() -> anyhow::Result<()> {
//! // Extract all IPs (default: includes private, loopback, broadcast)
//! let extractor = ExtractorBuilder::new().build()?;
//!
//! let input = b"Connect from 192.168.1.1 to 2001:db8::1";
//! for range in extractor.find_iter(input) {
//!     let ip = std::str::from_utf8(&input[range])?;
//!     println!("Found: {}", ip);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Tagging and Output
//!
//! For more structured output (e.g., JSON), use the `Tagged` and `Tag` types:
//!
//! ```no_run
//! use ip_extract::{ExtractorBuilder, Tagged, Tag};
//!
//! # fn main() -> anyhow::Result<()> {
//! let extractor = ExtractorBuilder::new().build()?;
//! let data = b"Server at 8.8.8.8";
//! let mut tagged = Tagged::new(data);
//!
//! for range in extractor.find_iter(data) {
//!     let ip = std::str::from_utf8(&data[range.clone()])?;
//!     let tag = Tag::new(ip).with_range(range);
//!     tagged = tagged.tag(tag);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Configuration
//!
//! Use `ExtractorBuilder` to filter specific IP categories:
//!
//! ```no_run
//! use ip_extract::ExtractorBuilder;
//!
//! # fn main() -> anyhow::Result<()> {
//! // Extract only publicly routable IPs
//! let extractor = ExtractorBuilder::new()
//!     .only_public()
//!     .build()?;
//!
//! // Or use granular control
//! let extractor = ExtractorBuilder::new()
//!     .ipv4(true)            // Extract IPv4 (default: true)
//!     .ipv6(false)           // Skip IPv6
//!     .ignore_private()      // Skip RFC 1918 ranges
//!     .ignore_loopback()     // Skip loopback (127.0.0.1, ::1)
//!     .build()?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Performance
//!
//! Typical throughput on modern hardware:
//! - Dense IPs (mostly IP addresses): **160+ MiB/s**
//! - Sparse logs (IPs mixed with text): **360+ MiB/s**
//! - No IPs (pure scanning): **620+ MiB/s**
//!
//! See `benches/ip_benchmark.rs` for details.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::ops::Range;
use std::sync::OnceLock;

use regex_automata::dfa::dense::DFA;
use regex_automata::dfa::Automaton;
use regex_automata::Input;

mod tag;
pub use tag::{Tag, Tagged, TextData};

// Alignment wrapper: guarantees u32 alignment for DFA deserialization.
// DFA::from_bytes() requires the byte slice to be u32-aligned; include_bytes!() only
// guarantees byte alignment. Wrapping in repr(C, align(4)) satisfies this at compile time,
// with zero runtime cost: no allocation, no copy, no Box::leak.
#[repr(C, align(4))]
struct AlignedDfa<T: ?Sized>(T);

static IPV4_DFA_BYTES: &AlignedDfa<[u8]> =
    &AlignedDfa(*include_bytes!(concat!(env!("OUT_DIR"), "/ipv4_only.dfa")));
static IPV6_DFA_BYTES: &AlignedDfa<[u8]> =
    &AlignedDfa(*include_bytes!(concat!(env!("OUT_DIR"), "/ipv6_only.dfa")));
static BOTH_DFA_BYTES: &AlignedDfa<[u8]> =
    &AlignedDfa(*include_bytes!(concat!(env!("OUT_DIR"), "/both.dfa")));

static DFA_IPV4: OnceLock<DFA<&'static [u32]>> = OnceLock::new();
static DFA_IPV6: OnceLock<DFA<&'static [u32]>> = OnceLock::new();
static DFA_BOTH: OnceLock<DFA<&'static [u32]>> = OnceLock::new();

fn load_dfa(aligned: &'static AlignedDfa<[u8]>) -> DFA<&'static [u32]> {
    let (dfa, _) = DFA::from_bytes(&aligned.0).expect("valid dfa from build.rs");
    dfa
}

fn get_ipv4_dfa() -> &'static DFA<&'static [u32]> {
    DFA_IPV4.get_or_init(|| load_dfa(IPV4_DFA_BYTES))
}
fn get_ipv6_dfa() -> &'static DFA<&'static [u32]> {
    DFA_IPV6.get_or_init(|| load_dfa(IPV6_DFA_BYTES))
}
fn get_both_dfa() -> &'static DFA<&'static [u32]> {
    DFA_BOTH.get_or_init(|| load_dfa(BOTH_DFA_BYTES))
}

#[derive(Clone, Debug)]
enum ValidatorType {
    IPv4 {
        include_private: bool,
        include_loopback: bool,
        include_broadcast: bool,
    },
    IPv6 {
        include_private: bool,
        include_loopback: bool,
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
            } => validate_ipv4(bytes, include_private, include_loopback, include_broadcast),
            ValidatorType::IPv6 {
                include_private,
                include_loopback,
            } => validate_ipv6(bytes, include_private, include_loopback),
        }
    }
}

/// The main IP address extractor.
///
/// An `Extractor` scans byte slices for IPv4 and/or IPv6 addresses, applying configurable
/// filters to include or exclude certain address classes (private, loopback, broadcast).
///
/// Extractors are best created via [`ExtractorBuilder`] and are designed to be reused
/// across many calls to `find_iter` for maximum efficiency.
///
/// # Bytes vs. Strings
///
/// This extractor works directly on byte slices rather than strings. This avoids UTF-8
/// validation overhead and enables zero-copy scanning of very large inputs.
///
/// # Performance
///
/// The extractor uses a compile-time DFA (Deterministic Finite Automaton) for O(n)
/// scanning with minimal overhead. See the crate-level documentation for throughput benchmarks.
pub struct Extractor {
    dfa: &'static DFA<&'static [u32]>,
    validators: Vec<ValidatorType>,
}

impl Extractor {
    /// Find all IP addresses in a byte slice.
    ///
    /// Returns an iterator of byte ranges `[start, end)` pointing to each IP address found.
    /// Ranges are guaranteed to be valid indices into `haystack`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ip_extract::ExtractorBuilder;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let extractor = ExtractorBuilder::new().build()?;
    /// let data = b"Log: 192.168.1.1 sent request to 8.8.8.8";
    ///
    /// for range in extractor.find_iter(data) {
    ///     let ip = std::str::from_utf8(&data[range]).unwrap();
    ///     println!("IP: {}", ip);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Arguments
    ///
    /// * `haystack` - A byte slice to search for IP addresses.
    ///
    /// # Returns
    ///
    /// An iterator yielding byte ranges for each valid IP address found.
    #[inline]
    pub fn find_iter<'a>(&'a self, haystack: &'a [u8]) -> impl Iterator<Item = Range<usize>> + 'a {
        let mut input = Input::new(haystack);

        std::iter::from_fn(move || {
            loop {
                // We use the specialized try_search_fwd method.
                // For a dense DFA, this is the core scanning loop.
                let Ok(Some(m)) = self.dfa.try_search_fwd(&input) else {
                    return None;
                };

                let end = m.offset();
                let pid = m.pattern().as_usize();
                let validator = &self.validators[pid];

                // Advance for next iteration regardless of whether this match is valid.
                input.set_start(end);

                // Walk backward from end to find the true start of the IP.
                // We know IPs are at most 39 bytes (IPv6 max), so cap the scan.
                // Stop as soon as we hit a non-IP character or the beginning of the buffer.
                let floor = end.saturating_sub(40);
                let start = (floor..end)
                    .rev()
                    .find(|&i| i == 0 || !is_ip_char(haystack[i - 1]))
                    .unwrap_or(floor);

                // Left boundary: the character before start must not be an IP char.
                // (The rev().find() above guarantees this by construction.)

                // Right boundary check: character after end must not continue the IP.
                let valid_right_boundary = match end.cmp(&haystack.len()) {
                    std::cmp::Ordering::Less => {
                        let next = haystack[end];
                        match validator {
                            ValidatorType::IPv4 { .. } => {
                                !(next.is_ascii_digit()
                                    || next == b'.'
                                        && end + 1 < haystack.len()
                                        && haystack[end + 1].is_ascii_digit())
                            }
                            ValidatorType::IPv6 { .. } => !is_ip_char(next),
                        }
                    }
                    _ => true,
                };

                if !valid_right_boundary {
                    continue;
                }

                // Single validate call — no loop, no multiple attempts.
                if validator.validate(&haystack[start..end]) {
                    return Some(start..end);
                }
            }
        })
    }
}

#[inline(always)]
fn is_ip_char(b: u8) -> bool {
    matches!(b, b'0'..=b'9' | b'a'..=b'f' | b'A'..=b'F' | b'.' | b':')
}

/// A builder for configuring IP extraction behavior.
///
/// Use `ExtractorBuilder` to specify which types of IP addresses should be extracted.
/// By default, it extracts both IPv4 and IPv6 but excludes private, loopback, and
/// broadcast addresses.
///
/// # Example
///
/// ```no_run
/// use ip_extract::ExtractorBuilder;
///
/// # fn main() -> anyhow::Result<()> {
/// let extractor = ExtractorBuilder::new()
///     .ipv4(true)
///     .ipv6(false)  // Only IPv4
///     .private_ips(true)  // Include private ranges
///     .build()?;
/// # Ok(())
/// # }
/// ```
pub struct ExtractorBuilder {
    include_ipv4: bool,
    include_ipv6: bool,
    include_private: bool,
    include_loopback: bool,
    include_broadcast: bool,
}

impl Default for ExtractorBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ExtractorBuilder {
    /// Create a new builder with default settings.
    ///
    /// By default, **all IP addresses are extracted** (principle of least surprise).
    /// Use `.only_public()` or `.ignore_*()` methods to filter specific categories.
    ///
    /// Defaults:
    /// - IPv4: enabled
    /// - IPv6: enabled
    /// - Private IPs: **enabled** (10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16, fc00::/7)
    /// - Loopback IPs: **enabled** (127.0.0.0/8, ::1)
    /// - Broadcast IPs: **enabled** (255.255.255.255, link-local)
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ip_extract::ExtractorBuilder;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// // Extract all IPs (default)
    /// let extractor = ExtractorBuilder::new().build()?;
    ///
    /// // Extract only public IPs
    /// let extractor = ExtractorBuilder::new().only_public().build()?;
    ///
    /// // Granular control
    /// let extractor = ExtractorBuilder::new()
    ///     .ignore_private()
    ///     .ignore_loopback()
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            include_ipv4: true,
            include_ipv6: true,
            include_private: true,
            include_loopback: true,
            include_broadcast: true,
        }
    }
    /// Enable or disable IPv4 address extraction.
    ///
    /// Default: `true`
    pub fn ipv4(&mut self, include: bool) -> &mut Self {
        self.include_ipv4 = include;
        self
    }

    /// Enable or disable IPv6 address extraction.
    ///
    /// Default: `true`
    pub fn ipv6(&mut self, include: bool) -> &mut Self {
        self.include_ipv6 = include;
        self
    }

    /// Include private IP addresses (RFC 1918 for IPv4, ULA for IPv6).
    ///
    /// Private ranges include:
    /// - IPv4: 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16
    /// - IPv6: fc00::/7 (ULA), fe80::/10 (link-local)
    ///
    /// Default: `true`
    pub fn private_ips(&mut self, include: bool) -> &mut Self {
        self.include_private = include;
        self
    }

    /// Include loopback addresses.
    ///
    /// Loopback ranges:
    /// - IPv4: 127.0.0.0/8
    /// - IPv6: ::1
    ///
    /// Default: `true`
    pub fn loopback_ips(&mut self, include: bool) -> &mut Self {
        self.include_loopback = include;
        self
    }

    /// Include broadcast addresses.
    ///
    /// Covers:
    /// - IPv4: 255.255.255.255 and link-local (169.254.0.0/16)
    /// - IPv6: link-local and other special ranges
    ///
    /// Default: `true`
    pub fn broadcast_ips(&mut self, include: bool) -> &mut Self {
        self.include_broadcast = include;
        self
    }

    /// Ignore private IP addresses (convenience for `.private_ips(false)`).
    ///
    /// Excludes:
    /// - IPv4: 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16
    /// - IPv6: fc00::/7 (ULA), fe80::/10 (link-local)
    pub fn ignore_private(&mut self) -> &mut Self {
        self.include_private = false;
        self
    }

    /// Ignore loopback addresses (convenience for `.loopback_ips(false)`).
    ///
    /// Excludes:
    /// - IPv4: 127.0.0.0/8
    /// - IPv6: ::1
    pub fn ignore_loopback(&mut self) -> &mut Self {
        self.include_loopback = false;
        self
    }

    /// Ignore broadcast addresses (convenience for `.broadcast_ips(false)`).
    ///
    /// Excludes:
    /// - IPv4: 255.255.255.255 and link-local (169.254.0.0/16)
    /// - IPv6: link-local and other special ranges
    pub fn ignore_broadcast(&mut self) -> &mut Self {
        self.include_broadcast = false;
        self
    }

    /// Extract only publicly routable IP addresses.
    ///
    /// This is a convenience method equivalent to:
    /// ```
    /// # use ip_extract::ExtractorBuilder;
    /// # let mut builder = ExtractorBuilder::new();
    /// builder
    ///     .ignore_private()
    ///     .ignore_loopback()
    ///     .ignore_broadcast();
    /// ```
    ///
    /// Excludes:
    /// - Private: RFC 1918 (IPv4), ULA (IPv6)
    /// - Loopback: 127.0.0.0/8, ::1
    /// - Broadcast: 255.255.255.255, link-local ranges
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ip_extract::ExtractorBuilder;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let extractor = ExtractorBuilder::new()
    ///     .only_public()
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn only_public(&mut self) -> &mut Self {
        self.include_private = false;
        self.include_loopback = false;
        self.include_broadcast = false;
        self
    }

    /// Build and return an `Extractor` with the configured settings.
    ///
    /// # Errors
    ///
    /// Returns an error if no IP version (IPv4 or IPv6) is enabled. At least one
    /// must be selected.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ip_extract::ExtractorBuilder;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let extractor = ExtractorBuilder::new()
    ///     .ipv4(true)
    ///     .ipv6(true)
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn build(&self) -> anyhow::Result<Extractor> {
        let (dfa, validators) = match (self.include_ipv4, self.include_ipv6) {
            (true, true) => (
                get_both_dfa(),
                vec![
                    ValidatorType::IPv4 {
                        include_private: self.include_private,
                        include_loopback: self.include_loopback,
                        include_broadcast: self.include_broadcast,
                    },
                    ValidatorType::IPv6 {
                        include_private: self.include_private,
                        include_loopback: self.include_loopback,
                    },
                ],
            ),
            (true, false) => (
                get_ipv4_dfa(),
                vec![ValidatorType::IPv4 {
                    include_private: self.include_private,
                    include_loopback: self.include_loopback,
                    include_broadcast: self.include_broadcast,
                }],
            ),
            (false, true) => (
                get_ipv6_dfa(),
                vec![ValidatorType::IPv6 {
                    include_private: self.include_private,
                    include_loopback: self.include_loopback,
                }],
            ),
            _ => anyhow::bail!("No IP address patterns selected"),
        };
        Ok(Extractor { dfa, validators })
    }
}

/// Validate an IPv4 address from a byte slice, applying filters.
///
/// This function uses `parse_ipv4_bytes` for strict validation and then checks
/// against the provided inclusion filters.
///
/// # Arguments
///
/// * `bytes` - Candidate byte slice to validate.
/// * `include_private` - Whether to include RFC 1918 addresses.
/// * `include_loopback` - Whether to include 127.0.0.0/8 addresses.
/// * `include_broadcast` - Whether to include broadcast and link-local addresses.
#[inline]
fn validate_ipv4(
    bytes: &[u8],
    include_private: bool,
    include_loopback: bool,
    include_broadcast: bool,
) -> bool {
    let Some(ipv4) = parse_ipv4_bytes(bytes) else {
        return false;
    };

    if !include_private && ipv4.is_private() {
        return false;
    }
    if !include_loopback && ipv4.is_loopback() {
        return false;
    }
    if !include_broadcast && (ipv4.is_broadcast() || ipv4.is_link_local()) {
        return false;
    }
    true
}

/// Extract all IPv4 and IPv6 addresses from input, returning them as strings.
///
/// This is a convenience function that uses default settings (all IP types included).
/// For more control, use `ExtractorBuilder` and `Extractor::find_iter()`.
///
/// # Errors
///
/// Returns an error if the builder fails to initialize (e.g., no IP types selected).
///
/// # Example
///
/// ```no_run
/// use ip_extract::extract;
///
/// # fn main() -> anyhow::Result<()> {
/// let ips = extract(b"Server at 192.168.1.1 and 2001:db8::1")?;
/// assert_eq!(ips, vec!["192.168.1.1", "2001:db8::1"]);
/// # Ok(())
/// # }
/// ```
pub fn extract(haystack: &[u8]) -> anyhow::Result<Vec<String>> {
    let extractor = ExtractorBuilder::new().build()?;
    Ok(extractor
        .find_iter(haystack)
        .map(|range| String::from_utf8_lossy(&haystack[range]).to_string())
        .collect())
}

/// Extract unique IPv4 and IPv6 addresses from input, returning them as strings.
///
/// Maintains order of first observation (not lexicographic order).
/// This is a convenience function that uses default settings (all IP types included).
/// For more control, use `ExtractorBuilder` and `Extractor::find_iter()`.
///
/// # Errors
///
/// Returns an error if the builder fails to initialize (e.g., no IP types selected).
///
/// # Example
///
/// ```no_run
/// use ip_extract::extract_unique;
///
/// # fn main() -> anyhow::Result<()> {
/// let ips = extract_unique(b"Server at 192.168.1.1, another at 192.168.1.1")?;
/// assert_eq!(ips, vec!["192.168.1.1"]);
/// # Ok(())
/// # }
/// ```
pub fn extract_unique(haystack: &[u8]) -> anyhow::Result<Vec<String>> {
    use std::collections::HashSet;

    let extractor = ExtractorBuilder::new().build()?;
    let mut seen = HashSet::new();
    let mut result = Vec::new();

    for range in extractor.find_iter(haystack) {
        let ip_str = String::from_utf8_lossy(&haystack[range]).to_string();
        if seen.insert(ip_str.clone()) {
            result.push(ip_str);
        }
    }

    Ok(result)
}

/// Extract all IPv4 and IPv6 addresses from input, returning them as parsed `IpAddr` objects.
///
/// This is a convenience function that uses default settings (all IP types included).
/// For more control, use `ExtractorBuilder` and `Extractor::find_iter()`.
///
/// # Errors
///
/// Returns an error if the builder fails to initialize (e.g., no IP types selected),
/// or if an extracted address cannot be parsed (should not happen in practice).
///
/// # Example
///
/// ```no_run
/// use ip_extract::extract_parsed;
///
/// # fn main() -> anyhow::Result<()> {
/// let ips = extract_parsed(b"Server at 192.168.1.1 and 2001:db8::1")?;
/// assert_eq!(ips.len(), 2);
/// assert!(ips[0].is_ipv4());
/// assert!(ips[1].is_ipv6());
/// # Ok(())
/// # }
/// ```
pub fn extract_parsed(haystack: &[u8]) -> anyhow::Result<Vec<IpAddr>> {
    let extractor = ExtractorBuilder::new().build()?;
    extractor
        .find_iter(haystack)
        .map(|range| {
            let s = std::str::from_utf8(&haystack[range])
                .map_err(|e| anyhow::anyhow!("Invalid UTF-8 in IP: {e}"))?;
            s.parse::<IpAddr>()
                .map_err(|e| anyhow::anyhow!("Failed to parse IP '{s}': {e}"))
        })
        .collect()
}

/// Extract unique IPv4 and IPv6 addresses from input, returning them as parsed `IpAddr` objects.
///
/// Maintains order of first observation (not lexicographic order).
/// This is a convenience function that uses default settings (all IP types included).
/// For more control, use `ExtractorBuilder` and `Extractor::find_iter()`.
///
/// # Errors
///
/// Returns an error if the builder fails to initialize (e.g., no IP types selected),
/// or if an extracted address cannot be parsed (should not happen in practice).
///
/// # Example
///
/// ```no_run
/// use ip_extract::extract_unique_parsed;
///
/// # fn main() -> anyhow::Result<()> {
/// let ips = extract_unique_parsed(b"Server at 192.168.1.1, another at 192.168.1.1")?;
/// assert_eq!(ips.len(), 1);
/// assert!(ips[0].is_ipv4());
/// # Ok(())
/// # }
/// ```
pub fn extract_unique_parsed(haystack: &[u8]) -> anyhow::Result<Vec<IpAddr>> {
    use std::collections::HashSet;

    let extractor = ExtractorBuilder::new().build()?;
    let mut seen = HashSet::new();
    let mut result = Vec::new();

    for range in extractor.find_iter(haystack) {
        let s = std::str::from_utf8(&haystack[range])
            .map_err(|e| anyhow::anyhow!("Invalid UTF-8 in IP: {e}"))?;
        let addr = s
            .parse::<IpAddr>()
            .map_err(|e| anyhow::anyhow!("Failed to parse IP '{s}': {e}"))?;
        if seen.insert(addr) {
            result.push(addr);
        }
    }

    Ok(result)
}

/// Parse an IPv4 address from a byte slice.
///
/// Performs strict validation of dotted-quad notation (e.g., `192.168.1.1`).
/// Rejects:
/// - Octet values > 255
/// - Leading zeros (e.g., `192.168.001.1`)
/// - Invalid formats
///
/// # Example
///
/// ```
/// use ip_extract::parse_ipv4_bytes;
///
/// assert_eq!(parse_ipv4_bytes(b"192.168.1.1"), Some("192.168.1.1".parse().unwrap()));
/// assert_eq!(parse_ipv4_bytes(b"256.1.1.1"), None);  // Out of range
/// assert_eq!(parse_ipv4_bytes(b"192.168.01.1"), None);  // Leading zero
/// ```
#[must_use]
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
        match b {
            b'.' => {
                if digits_in_octet == 0 || octet_idx == 3 {
                    return None;
                }
                #[allow(clippy::cast_possible_truncation)]
                {
                    octets[octet_idx] = current_val as u8;
                }
                octet_idx += 1;
                current_val = 0;
                digits_in_octet = 0;
            }
            b'0'..=b'9' => {
                let digit = u16::from(b - b'0');
                if digits_in_octet > 0 && current_val == 0 {
                    return None;
                }
                current_val = current_val * 10 + digit;
                if current_val > 255 {
                    return None;
                }
                digits_in_octet += 1;
            }
            _ => return None,
        }
    }
    if octet_idx != 3 || digits_in_octet == 0 {
        return None;
    }
    #[allow(clippy::cast_possible_truncation)]
    {
        octets[3] = current_val as u8;
    }
    Some(Ipv4Addr::new(octets[0], octets[1], octets[2], octets[3]))
}

/// Check if an IPv6 address is a Unique Local Address (ULA) per RFC 4193.
/// ULA addresses are in the fc00::/7 range (fc00:: to fdff::).
#[inline]
fn is_unique_local(ip: &Ipv6Addr) -> bool {
    matches!(ip.octets()[0], 0xfc | 0xfd)
}

/// Validate an IPv6 address from a byte slice, applying filters.
///
/// This function performs parsing and category-based filtering. It uses
/// `unsafe` `from_utf8_unchecked` for performance, as the candidates are
/// already filtered by the DFA for IP-like characters.
///
/// # Arguments
///
/// * `bytes` - Candidate byte slice to validate.
/// * `include_private` - Whether to include ULA and link-local addresses.
/// * `include_loopback` - Whether to include the loopback address (`::1`).
#[inline]
fn validate_ipv6(bytes: &[u8], include_private: bool, include_loopback: bool) -> bool {
    if bytes.len() < 2 {
        return false;
    }
    let s = unsafe { std::str::from_utf8_unchecked(bytes) };
    let Ok(ip) = s.parse::<IpAddr>() else {
        return false;
    };

    match ip {
        IpAddr::V6(ipv6) => {
            if !include_private && (ipv6.is_unicast_link_local() || is_unique_local(&ipv6)) {
                return false;
            }
            if !include_loopback && ipv6.is_loopback() {
                return false;
            }
            true
        }
        IpAddr::V4(_) => false,
    }
}

impl std::fmt::Debug for Extractor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Extractor")
            .field("validators", &self.validators)
            .finish()
    }
}
