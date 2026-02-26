use std::env;
use std::fs;
use std::path::Path;

use regex_automata::dfa::dense::DFA;
use regex_automata::nfa::thompson;
use regex_automata::MatchKind;

/// IPv4 address pattern: matches a complete dotted-quad notation.
///
/// Each octet is validated to be 0-255:
/// - 25[0-5]      matches 250-255
/// - 2[0-4][0-9]  matches 200-249
/// - [01]?[0-9][0-9]? matches 0-199
///
/// The pattern matches anywhere in text; boundary validation is done in lib.rs.
/// Note: regex-automata doesn't support lookahead/lookbehind, so right boundary
/// checking happens in the backtracking algorithm (see `is_ip_char` in lib.rs).
static IPV4_PATTERN: &str = r"(?x)
  (?:^|[^0-9A-Fa-f:.])
  (?:
    (?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)  # First octet
    \.
    (?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)  # Second octet
    \.
    (?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)  # Third octet
    \.
    (?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)  # Fourth octet
  )
";

/// IPv6 address pattern: matches various IPv6 formats.
///
/// Supports:
/// - Full form: 2001:db8:85a3:0:0:8a2e:370:7334 (39 chars max)
/// - Compressed: 2001:db8::1, ::1, :: (2 chars min)
/// - IPv4-mapped: ::ffff:192.0.2.1
///
/// Does NOT support zone IDs (fe80::1%eth0) to keep the implementation simple.
/// Boundary validation is done in lib.rs.
static IPV6_PATTERN: &str = r"(?x)
  (?:
    # IPv4-embedded IPv6 with leading segments
    (?:(?:[0-9a-fA-F]){1,4}:){1,4}:[^\s:]
    (?:
      (?:(?:25[0-5]|(?:2[0-4]|1{0,1}[0-9]){0,1}[0-9])\.){3,3}
      (?:25[0-5]|(?:2[0-4]|1{0,1}[0-9]){0,1}[0-9])
    )
  |
    # IPv4-mapped or IPv4-compatible IPv6
    ::(?:ffff(?::0{1,4}){0,1}:){0,1}[^\s:]
    (?:
      (?:(?:25[0-5]|(?:2[0-4]|1{0,1}[0-9]){0,1}[0-9])\.){3,3}
      (?:25[0-5]|(?:2[0-4]|1{0,1}[0-9]){0,1}[0-9])
    )
  |
    # Compressed form starting with ::
    :(?:(?::(?:(?:[0-9a-fA-F]){1,4})){1,7}|:)
  |
    # Standard forms with various compressions
    (?:(?:[0-9a-fA-F]){1,4}):(?:(?::(?:(?:[0-9a-fA-F]){1,4})){1,6})
  |
    (?:(?:(?:[0-9a-fA-F]){1,4}):){1,2}(?::(?:(?:[0-9a-fA-F]){1,4})){1,5}
  |
    (?:(?:(?:[0-9a-fA-F]){1,4}):){1,3}(?::(?:(?:[0-9a-fA-F]){1,4})){1,4}
  |
    (?:(?:(?:[0-9a-fA-F]){1,4}):){1,4}(?::(?:(?:[0-9a-fA-F]){1,4})){1,3}
  |
    (?:(?:(?:[0-9a-fA-F]){1,4}):){1,5}(?::(?:(?:[0-9a-fA-F]){1,4})){1,2}
  |
    (?:(?:(?:[0-9a-fA-F]){1,4}):){1,6}:(?:(?:[0-9a-fA-F]){1,4})
  |
    (?:(?:(?:[0-9a-fA-F]){1,4}):){1,7}:
  |
    (?:(?:(?:[0-9a-fA-F]){1,4}):){7,7}(?:(?:[0-9a-fA-F]){1,4})
  )
";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=build.rs");

    let out_dir = env::var_os("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir);

    // Build only Forward DFAs to minimize binary size overhead.
    // We can efficiently find the start position in code since IPs have
    // clear boundaries and a strictly limited character set.
    build_and_save(&[IPV4_PATTERN], "ipv4_only", out_dir)?;
    build_and_save(&[IPV6_PATTERN], "ipv6_only", out_dir)?;
    build_and_save(&[IPV4_PATTERN, IPV6_PATTERN], "both", out_dir)?;

    Ok(())
}

fn build_and_save(
    patterns: &[&str],
    filename_base: &str,
    out_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let dfa = DFA::builder()
        .thompson(thompson::Config::new().shrink(true))
        .configure(
            DFA::config()
                .match_kind(MatchKind::LeftmostFirst)
                .minimize(true),
        )
        .build_many(patterns)?;

    let (bytes, _) = dfa.to_bytes_little_endian();
    fs::write(out_dir.join(format!("{filename_base}.dfa")), &bytes)?;

    Ok(())
}
