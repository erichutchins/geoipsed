# ipextract

High-performance IP address extraction for Python, powered by standard-compliant Rust DFAs.

`ipextract` is a Python binding for the [`ip-extract`](https://github.com/erichutchins/geoipsed/tree/main/crates/ip-extract) Rust crate. It provides O(n) scanning of unstructured text to find IPv4 and IPv6 addresses without the overhead and backtracking of regex engines.

## Installation

```bash
pip install ipextract
```

## Quick Start

```python
import ipextract

# Extract all IPs from a string
ips = ipextract.extract("Connection from 192.168.1.1 and 8.8.8.8")
# ["192.168.1.1", "8.8.8.8"]

# Deduplicate, preserving first-seen order
ips = ipextract.extract_unique("8.8.8.8 1.1.1.1 8.8.8.8")
# ["8.8.8.8", "1.1.1.1"]

# bytes input works too (faster for large log files)
ips = ipextract.extract(b"host 10.0.0.1 connected")
# ["10.0.0.1"]
```

## Performance

`ipextract` is significantly faster than native Python regex for scanning logs and large files because it uses a compiled DFA (Deterministic Finite Automaton).

Compared against Python `re` + `ipaddress.ip_address()` validation (a fair apples-to-apples comparison where both sides extract and validate):

| Scenario | `re` + `ipaddress` (ms) | `ipextract` (ms) | Speedup |
| :--- | :---: | :---: | :---: |
| **Dense IPs** (1000 mixed v4+v6) | 2.3ms | 0.25ms | **9x** |
| **Sparse Logs** (1000 IPs in noise) | 7.4ms | 0.46ms | **16x** |
| **Pure Text** (100KB with zero IPs) | 4.0ms | 0.16ms | **25x** |
| **Defanged IPs** (1000 mixed) | 2.5ms | 0.35ms | **7x** |

## Features

- **Blazing Fast**: Up to 25x faster than regex for negative scanning (text with no IPs).
- **Configurable**: Filter for public-only, ignore loopback, or skip IPv6 entirely.
- **Zero Dependencies**: Ships as a self-contained wheel.
- **Strict**: Eliminates false positives like `1.2.3.4.5` or `123.456.78.9`.
- **Defang Support**: Automatically handles and normalizes defanged IPs like `192[.]168[.]1[.]1`.

## Advanced Usage

For processing many lines, create a reusable `Extractor`:

```python
# Configure once
extractor = ipextract.Extractor(private=False, ipv6=False)

# Reuse many times to avoid initialization overhead
for line in log_lines:
    ips = extractor.extract(line)
```

### Configuration Options

| Method                 | Description                                       |
| ---------------------- | ------------------------------------------------- |
| `.only_public()`       | Exclude private, loopback, and broadcast          |
| `.ignore_private()`    | Exclude RFC 1918 (IPv4) and ULA/link-local (IPv6) |
| `.ignore_loopback()`   | Exclude 127.0.0.0/8 and ::1                       |
| `.ignore_broadcast()`  | Exclude 255.255.255.255 and link-local ranges     |
| `.ipv4(False)`         | Skip IPv4 entirely                                |
| `.ipv6(False)`         | Skip IPv6 entirely                                |

## License

MIT OR Unlicense
