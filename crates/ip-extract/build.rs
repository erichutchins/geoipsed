use std::env;
use std::fs;
use std::path::Path;

use regex_automata::dfa::dense::DFA;
use regex_automata::nfa::thompson;
use regex_automata::MatchKind;

/// Defanged IPv4 pattern: matches both normal dots and `[.]` bracket notation.
///
/// Each dot separator is expanded to `(?:\.|\[\.\])` to match either form.
/// This is the only IPv4 pattern — the expanded DFA has no measurable regression
/// on normal input and is 22% faster on defanged input.
static IPV4_DEFANG_PATTERN: &str = r"(?x)
  (?:^|[^0-9A-Fa-f:.])
  (?:
    (?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)  # First octet
    (?:\.|\[\.\])
    (?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)  # Second octet
    (?:\.|\[\.\])
    (?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)  # Third octet
    (?:\.|\[\.\])
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

    // Defang-aware patterns are the only patterns. The expanded DFA is the
    // default — it has no measurable regression on normal input and is 22%
    // faster on defanged input (see docs/plans/2026-02-28-defang-support-design.md).
    let ipv6_defang = defangify_colons(IPV6_PATTERN);

    build_and_save(&[IPV4_DEFANG_PATTERN], "ipv4", out_dir)?;
    build_and_save(&[&ipv6_defang], "ipv6", out_dir)?;
    build_and_save(&[IPV4_DEFANG_PATTERN, &ipv6_defang], "both", out_dir)?;

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

/// Transform a regex pattern to also match defanged IPv6 colon notation.
///
/// Expands `:` → `(?::|\[:\])` and `::` → `(?:::|\[::\])`.
/// Correctly skips colons inside character classes `[...]` and the `?:` non-capturing
/// group marker. Handles `(?x)` verbose mode by skipping `#` line comments.
fn defangify_colons(pattern: &str) -> String {
    let bytes = pattern.as_bytes();
    let mut out = String::with_capacity(pattern.len() * 2);
    let mut i = 0;
    let mut in_char_class = false;
    let mut in_comment = false;

    while i < bytes.len() {
        let b = bytes[i];

        // Newline ends a verbose-mode comment
        if b == b'\n' {
            in_comment = false;
            out.push('\n');
            i += 1;
            continue;
        }

        // Inside a comment: pass through verbatim
        if in_comment {
            out.push(b as char);
            i += 1;
            continue;
        }

        match b {
            b'#' if !in_char_class => {
                in_comment = true;
                out.push('#');
                i += 1;
            }
            b'[' if !in_char_class => {
                in_char_class = true;
                out.push('[');
                i += 1;
            }
            b']' if in_char_class => {
                in_char_class = false;
                out.push(']');
                i += 1;
            }
            b':' if !in_char_class => {
                // The `:` in `?:` is the non-capturing group marker — pass through.
                if i > 0 && bytes[i - 1] == b'?' {
                    out.push(':');
                    i += 1;
                    continue;
                }
                // Double colon `::` → `(?:::|\[::\])`
                if i + 1 < bytes.len() && bytes[i + 1] == b':' {
                    out.push_str(r"(?:::|\[::\])");
                    i += 2;
                } else {
                    // Single colon `:` → `(?::|\[:\])`
                    out.push_str(r"(?::|\[:\])");
                    i += 1;
                }
            }
            _ => {
                out.push(b as char);
                i += 1;
            }
        }
    }

    out
}
