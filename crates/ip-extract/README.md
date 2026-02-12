# ip-extract

A high-performance IP address extraction engine for Rust.

This crate provides a fast, robust, and configurable way to find IPv4 and IPv6 addresses in text. It is used as the core extraction engine for `geoipsed`.

## Features

- **Blazing Fast**: Uses `regex-automata` for high-throughput scanning.
- **Strict Validation**: Performs deep validation of IP address candidates to eliminate false positives.
- **Configurable**: Easily include/exclude private, loopback, or broadcast addresses.
- **Byte-Oriented**: Operates directly on byte slices for maximum performance and zero UTF-8 overhead.
- **Aho-Corasick Optimized**: Leverages pre-filters for even faster matching in common cases.

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
ip-extract = { path = "crates/ip-extract" } # Adjust path as needed
```

### Basic Example

```rust
use ip_extract::ExtractorBuilder;

fn main() -> anyhow::Result<()> {
    let extractor = ExtractorBuilder::new()
        .ipv4(true)
        .ipv6(true)
        .private_ips(true)
        .build()?;

    let input = b"Connection from 192.168.1.1 and 8.8.8.8";
    
    for range in extractor.find_iter(input) {
        let ip = std::str::from_utf8(&input[range]).unwrap();
        println!("Found IP: {}", ip);
    }
    
    Ok(())
}
```

## Performance Architecture

`ip-extract` is built for maximum throughput:
- **Compile-Time DFA**: The regex patterns are compiled into a dense Forward DFA during the build process. Initialization is zero-cost, and scanning is O(n) with no runtime backtracking.
- **Strict Boundary Logic**: The engine ensures matched IPs are not sub-tokens of larger IP-like garbage (e.g., `1.2.3.4.5` is correctly ignored).
- **Static Footprint**: The pre-compiled DFA is embedded in the binary (~600KB), eliminating heap allocation for regex state during scanning.

## Limitations

To maintain extreme performance, this engine has a few intentional constraints:
- **No IPv6 Scope IDs**: Formats like `fe80::1%eth0` are considered out of scope and will not be matched.
- **Strict Character Boundaries**: Matches are expected to be separated by non-IP characters (spaces, punctuation, etc.). Very tightly packed or malformed IP sequences might be skipped to ensure zero false positives.
- **Strict IPv4**: Only standard four-octet dotted notation is supported (e.g., `1.2.3.4`).

This two-stage approach (Static DFA + Targeted Validation) ensures that the scanner remains extremely fast while maintaining high accuracy.
