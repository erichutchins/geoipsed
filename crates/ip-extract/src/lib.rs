//! High-performance IP address extraction and tagging engine.

use std::net::{IpAddr, Ipv4Addr};
use std::ops::Range;
use std::sync::OnceLock;

use regex_automata::dfa::dense::DFA;
use regex_automata::dfa::Automaton;
use regex_automata::Input;

mod tag;
pub use tag::{Tag, Tagged, TextData};

// Pre-compiled DFA bytes
static IPV4_DFA_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/ipv4_only.dfa"));
static IPV6_DFA_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/ipv6_only.dfa"));
static BOTH_DFA_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/both.dfa"));

static DFA_IPV4: OnceLock<&'static DFA<&'static [u32]>> = OnceLock::new();
static DFA_IPV6: OnceLock<&'static DFA<&'static [u32]>> = OnceLock::new();
static DFA_BOTH: OnceLock<&'static DFA<&'static [u32]>> = OnceLock::new();

fn load_dfa(bytes: &'static [u8]) -> &'static DFA<&'static [u32]> {
    // DFA requires u32-aligned data. We allocate a u32 buffer, copy bytes into it,
    // then leak it to get 'static lifetime for zero-cost deserialization.
    let len = bytes.len();
    let cap = len.div_ceil(4);
    let mut storage = vec![0u32; cap];
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), storage.as_mut_ptr() as *mut u8, len);
    }
    let storage_ref: &'static mut [u32] = Box::leak(storage.into_boxed_slice());
    let aligned_slice =
        unsafe { std::slice::from_raw_parts(storage_ref.as_ptr() as *const u8, len) };
    let (dfa, _) = DFA::from_bytes(aligned_slice).expect("valid dfa from build.rs");
    Box::leak(Box::new(dfa))
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
            } => validate_ipv4(
                bytes,
                include_private,
                include_loopback,
                include_broadcast,
                only_routable,
            ),
            ValidatorType::IPv6 {
                include_private,
                include_loopback,
                only_routable,
            } => validate_ipv6(bytes, include_private, include_loopback, only_routable),
        }
    }
}

pub struct Extractor {
    dfa: &'static DFA<&'static [u32]>,
    validators: Vec<ValidatorType>,
}

impl Extractor {
    #[inline]
    pub fn find_iter<'a>(&'a self, haystack: &'a [u8]) -> impl Iterator<Item = Range<usize>> + 'a {
        let mut input = Input::new(haystack);

        std::iter::from_fn(move || {
            loop {
                // We use the specialized try_search_fwd method.
                // For a dense DFA, this is the core scanning loop.
                let m = match self.dfa.try_search_fwd(&input) {
                    Ok(Some(m)) => m,
                    _ => return None,
                };

                let end = m.offset();
                let pid = m.pattern().as_usize();
                let validator = &self.validators[pid];

                // Backtrack to find the start. IPv6 with zone ID is max ~45 bytes.
                let mut start_scan = end.saturating_sub(46);
                while start_scan < end && !is_ip_char(haystack[start_scan]) {
                    start_scan += 1;
                }

                let mut actual_start = None;
                for s in start_scan..end {
                    if s > 0 && is_ip_char(haystack[s - 1]) {
                        continue;
                    }
                    if validator.validate(&haystack[s..end]) {
                        if end < haystack.len() && is_ip_char(haystack[end]) {
                            break;
                        }
                        actual_start = Some(s);
                        break;
                    }
                }

                // Advance input.
                input.set_start(end);

                if let Some(s) = actual_start {
                    return Some(s..end);
                }

                if end >= haystack.len() {
                    return None;
                }
            }
        })
    }
}

#[inline(always)]
fn is_ip_char(b: u8) -> bool {
    matches!(b, b'0'..=b'9' | b'a'..=b'f' | b'A'..=b'F' | b'.' | b':' | b'%')
}

pub struct ExtractorBuilder {
    include_ipv4: bool,
    include_ipv6: bool,
    include_private: bool,
    include_loopback: bool,
    include_broadcast: bool,
    only_routable: bool,
}

impl Default for ExtractorBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ExtractorBuilder {
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
    pub fn ipv4(&mut self, include: bool) -> &mut Self {
        self.include_ipv4 = include;
        self
    }
    pub fn ipv6(&mut self, include: bool) -> &mut Self {
        self.include_ipv6 = include;
        self
    }
    pub fn private_ips(&mut self, include: bool) -> &mut Self {
        self.include_private = include;
        self
    }
    pub fn loopback_ips(&mut self, include: bool) -> &mut Self {
        self.include_loopback = include;
        self
    }
    pub fn broadcast_ips(&mut self, include: bool) -> &mut Self {
        self.include_broadcast = include;
        self
    }
    pub fn only_routable(&mut self, only: bool) -> &mut Self {
        self.only_routable = only;
        self
    }

    pub fn build(&self) -> anyhow::Result<Extractor> {
        let (dfa, validators) = match (self.include_ipv4, self.include_ipv6) {
            (true, true) => (
                get_both_dfa(),
                vec![
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
                ],
            ),
            (true, false) => (
                get_ipv4_dfa(),
                vec![ValidatorType::IPv4 {
                    include_private: self.include_private,
                    include_loopback: self.include_loopback,
                    include_broadcast: self.include_broadcast,
                    only_routable: self.only_routable,
                }],
            ),
            (false, true) => (
                get_ipv6_dfa(),
                vec![ValidatorType::IPv6 {
                    include_private: self.include_private,
                    include_loopback: self.include_loopback,
                    only_routable: self.only_routable,
                }],
            ),
            _ => anyhow::bail!("No IP address patterns selected"),
        };
        Ok(Extractor { dfa, validators })
    }
}

#[inline]
fn validate_ipv4(
    bytes: &[u8],
    include_private: bool,
    include_loopback: bool,
    include_broadcast: bool,
    _only_routable: bool,
) -> bool {
    let ipv4 = match parse_ipv4_bytes(bytes) {
        Some(ip) => ip,
        None => return false,
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
                octets[octet_idx] = current_val as u8;
                octet_idx += 1;
                current_val = 0;
                digits_in_octet = 0;
            }
            b'0'..=b'9' => {
                let digit = (b - b'0') as u16;
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
    octets[3] = current_val as u8;
    Some(Ipv4Addr::new(octets[0], octets[1], octets[2], octets[3]))
}

#[inline]
fn validate_ipv6(
    bytes: &[u8],
    include_private: bool,
    include_loopback: bool,
    _only_routable: bool,
) -> bool {
    if bytes.len() < 2 {
        return false;
    }
    let s = unsafe { std::str::from_utf8_unchecked(bytes) };
    let ip = match s.parse::<IpAddr>() {
        Ok(ip) => ip,
        Err(_) => return false,
    };
    match ip {
        IpAddr::V6(ipv6) => {
            if !include_private && ipv6.is_unicast_link_local() {
                return false;
            }
            if !include_loopback && ipv6.is_loopback() {
                return false;
            }
            true
        }
        _ => false,
    }
}

impl std::fmt::Debug for Extractor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Extractor")
            .field("validators", &self.validators)
            .finish()
    }
}
