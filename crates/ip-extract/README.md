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

## How it Works

`ip-extract` combines powerful regular expression matching with manual validation. While the regex identifies potential IP-like strings, the internal validator checks for:
- Proper octet ranges (0-255).
- Leading zero restrictions (matching `std::net::Ipv4Addr` behavior).
- IPv6 formatting rules.
- Specific address types (private, loopback, etc.) based on your configuration.

This two-stage approach ensures that the scanner remains extremely fast while maintaining high accuracy.
