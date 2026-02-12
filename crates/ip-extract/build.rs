use std::env;
use std::fs;
use std::path::Path;

use regex_automata::dfa::dense::DFA;
use regex_automata::MatchKind;

// IP pattern strings
static IPV4_PATTERN: &str =
    r"(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)";
static IPV6_PATTERN: &str = r"(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){1,4}:[^\s:](?:(?:(?:25[0-5]|(?:2[0-4]|1{0,1}[0-9]){0,1}[0-9]).){3,3}(?:25[0-5]|(?:2[0-4]|1{0,1}[0-9]){0,1}[0-9])))|(?:::(?:ffff(?::0{1,4}){0,1}:){0,1}[^\s:](?:(?:(?:25[0-5]|(?:2[0-4]|1{0,1}[0-9]){0,1}[0-9]).){3,3}(?:25[0-5]|(?:2[0-4]|1{0,1}[0-9]){0,1}[0-9])))|(?:fe80:(?::(?:(?:[0-9a-fA-F]){1,4})){0,4}%[0-9a-zA-Z]{1,})|(?::(?:(?::(?:(?:[0-9a-fA-F]){1,4})){1,7}|:))|(?:(?:(?:[0-9a-fA-F]){1,4}):(?:(?::(?:(?:[0-9a-fA-F]){1,4})){1,6}))|(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){1,2}(?::(?:(?:[0-9a-fA-F]){1,4})){1,5})|(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){1,3}(?::(?:(?:[0-9a-fA-F]){1,4})){1,4})|(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){1,4}(?::(?:(?:[0-9a-fA-F]){1,4})){1,3})|(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){1,5}(?::(?:(?:[0-9a-fA-F]){1,4})){1,2})|(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){1,6}:(?:(?:[0-9a-fA-F]){1,4}))|(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){1,7}:)|(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){7,7}(?:(?:[0-9a-fA-F]){1,4}))";

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
        .configure(
            DFA::config()
                .match_kind(MatchKind::LeftmostFirst)
                .minimize(true),
        )
        .build_many(patterns)?;

    let (bytes, _) = dfa.to_bytes_little_endian();
    fs::write(out_dir.join(format!("{}.dfa", filename_base)), &bytes)?;

    Ok(())
}
