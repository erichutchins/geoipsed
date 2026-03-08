# ip-extract

A fast IP address extraction library for Rust.

Extract IPv4 and IPv6 addresses from unstructured text with minimal overhead. This crate powers the core extraction engine for `geoipsed` and is designed for high-throughput scanning of large datasets.

## Features

- ⚡ **Performance Optimized**: Compile-time DFA with O(n) scanning, no runtime regex compilation
- 🎯 **Strict Validation**: Deep validation eliminates false positives (e.g., rejects `1.2.3.4.5`)
- 🛡️ **Defang Support**: Automatically matches defanged IPs (`192[.]168[.]1[.]1`, `2001[:]db8[:]...`) with negligible overhead
- ⚙️ **Configurable**: Fine-grained control over address types (private, loopback, broadcast)
- 🔢 **Byte-Oriented**: Zero-copy scanning directly on byte slices, no UTF-8 validation overhead

### Basic Example

Extract all IP addresses (default behavior):

```rust
use ip_extract::ExtractorBuilder;

fn main() -> anyhow::Result<()> {
    // Extracts all IPs: IPv4, IPv6, private, loopback, broadcast
    let extractor = ExtractorBuilder::new().build()?;

    let input = b"Connection from 192.168.1.1 and 8.8.8.8";

    for range in extractor.find_iter(input) {
        let ip = std::str::from_utf8(&input[range]).unwrap();
        println!("Found IP: {}", ip);
    }

    Ok(())
}
```

### Configuration Examples

```rust
use ip_extract::ExtractorBuilder;

// Extract only public IPs (recommended for most use cases)
let extractor = ExtractorBuilder::new()
    .only_public()
    .build()?;

// Extract only IPv4, ignoring loopback
let extractor = ExtractorBuilder::new()
    .ipv6(false)
    .ignore_loopback()
    .build()?;

// Fine-grained control
let extractor = ExtractorBuilder::new()
    .ipv4(true)
    .ipv6(true)
    .ignore_private()
    .ignore_broadcast()
    .build()?;
```

## Defanged IP Support

Threat intelligence reports and security logs commonly use "defanged" IPs to prevent accidental connections. `ip-extract` recognizes these automatically — no opt-in needed.

```rust
let extractor = ExtractorBuilder::new().build()?;

let input = b"IOC: 192[.]168[.]1[.]1 and 2001[:]db8[:]0[:]0[:]0[:]0[:]0[:]1";
for m in extractor.match_iter(input) {
    // as_str() returns the normalized (refanged) IP — zero-copy for normal input
    println!("{}", m.as_str());         // "192.168.1.1"

    // as_matched_str() returns exactly what was in the input
    println!("{}", m.as_matched_str()); // "192[.]168[.]1[.]1"

    // ip() parses to std::net::IpAddr
    println!("{:?}", m.ip());           // Ok(V4(192.168.1.1))
}
```

### Supported notation

| Type | Bracket | Example |
| :--- | :---: | :--- |
| IPv4 | `[.]` | `192[.]168[.]1[.]1` |
| IPv6 | `[:]` | `2001[:]db8[:]0[:]0[:]0[:]0[:]0[:]1` |

**Note:** IPv6 defanged notation requires fully-expanded form — `[::]` compression is not supported.

### Performance impact

Defang patterns are expanded into the DFA at compile time (+3KB binary size). There is **no measurable regression on normal (fanged) input**. On defanged input, the DFA approach is 16% faster than pre-processing normalization.

## Benchmarks

Typical throughput on modern hardware:

| Scenario                        | Throughput     |
| ------------------------------- | -------------- |
| Dense IPs (mostly IP addresses) | **160+ MiB/s** |
| Sparse logs (mixed with text)   | **360+ MiB/s** |
| Pure scanning (no IPs)          | **620+ MiB/s** |

## Performance Architecture

`ip-extract` achieves maximum throughput through a two-stage design:

1. **Compile-Time DFA** (Build Phase)
   - Regex patterns compiled into dense Forward DFAs during build
   - DFA serialized and embedded in binary (~600KB)
   - Eliminates all runtime regex compilation

2. **Zero-Cost Scanning** (Runtime)
   - O(n) byte scanning with lazy DFA initialization
   - Single forward pass, no backtracking
   - Validation only on candidates, not all scanned bytes

3. **Strict Validation**
   - Hand-optimized[^1] IPv4 parser (20-30% faster than `std::net`)
   - Boundary checking prevents false matches (e.g., `1.2.3.4.5` rejected)
   - Configurable filters for special ranges

[^1]: AI wrote all of this. It does not have hands.
