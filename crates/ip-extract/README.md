# ip-extract

A fast IP address extraction library for Rust.

Extract IPv4 and IPv6 addresses from unstructured text with minimal overhead. This crate powers the core extraction engine for [`geoipsed`](https://github.com/erichutchins/geoipsed) and is designed for high-throughput scanning of large datasets.

## Features

- ⚡ **Performance Optimized**: Compile-time DFA with O(n) scanning, no runtime regex compilation
- 🎯 **Strict Validation**: Deep validation eliminates false positives (e.g., rejects `1.2.3.4.5`)
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

## Benchmarks

Typical throughput on modern hardware (see `benches/ip_benchmark.rs`):

| Scenario                        | Throughput     |
| ------------------------------- | -------------- |
| Dense IPs (mostly IP addresses) | **160+ MiB/s** |
| Sparse logs (mixed with text)   | **360+ MiB/s** |
| Pure scanning (no IPs)          | **620+ MiB/s** |

Run benchmarks locally:

```bash
cargo bench --bench ip_benchmark
```

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

## API Defaults

By default, `ExtractorBuilder::new()` extracts **all IP addresses**:

- ✅ **IPv4**: Enabled
- ✅ **IPv6**: Enabled
- ✅ **Private IPs**: Enabled (RFC 1918, IPv6 ULA)
- ✅ **Loopback**: Enabled (127.0.0.0/8, ::1)
- ✅ **Broadcast**: Enabled (255.255.255.255, link-local)

Use convenience methods to filter:

- `.only_public()` - Extract only publicly routable IPs
- `.ignore_private()` - Skip RFC 1918 and IPv6 ULA ranges
- `.ignore_loopback()` - Skip loopback addresses
- `.ignore_broadcast()` - Skip broadcast addresses

## Limitations

By design, this engine makes conservative choices for performance:

- **Strict Boundaries**: IPs must be separated by non-IP characters; concatenated IPs without separators may be skipped
- **Standard IPv4 Only**: Four-octet dotted notation only (e.g., `192.168.0.1` only, not `0xC0A80001`, not `3232235521`, and not `11000000.10101000.00000000.00000001`)
- **IPv6 Scope IDs Not Captured**: Formats like `fe80::1%eth0` will extract as `fe80::1` (the scope ID `%eth0` is treated as a boundary and dropped).

These constraints ensure minimal false positives and maximum scanning performance.

[^1]: AI wrote all of this. It does not have hands.
