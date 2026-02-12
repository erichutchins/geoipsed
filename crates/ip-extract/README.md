# ip-extract

A **blazingly fast** IP address extraction library for Rust.

Extract IPv4 and IPv6 addresses from unstructured text with minimal overhead. This crate powers the core extraction engine for [`geoipsed`](https://github.com/erichutchins/geoipsed) and is designed for high-throughput scanning of large datasets.

## Features

- ⚡ **Blazing Fast**: Compile-time DFA with O(n) scanning, no runtime regex compilation
- 🎯 **Strict Validation**: Deep validation eliminates false positives (e.g., rejects `1.2.3.4.5`)
- ⚙️ **Configurable**: Fine-grained control over address types (private, loopback, broadcast)
- 🔢 **Byte-Oriented**: Zero-copy scanning directly on byte slices, no UTF-8 validation overhead

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
   - Hand-optimized IPv4 parser (faster than `std::net`)
   - Boundary checking prevents false matches (e.g., `1.2.3.4.5` rejected)
   - Configurable filters for special ranges

## Limitations

By design, this engine makes conservative choices for performance:

- **No IPv6 Scope IDs**: Formats like `fe80::1%eth0` not supported (rare in practice)
- **Strict Boundaries**: IPs must be separated by non-IP characters; concatenated IPs without separators may be skipped
- **Standard IPv4 Only**: Four-octet dotted notation only (e.g., `192.168.1.1`)

These constraints ensure zero false positives and maximum scanning performance.
