# Python: ipextract

A Python package for fast IP address extraction from text, powered by the `ip-extract` Rust crate via PyO3.

## Installation

```bash
pip install ipextract
```

Requires Python 3.10+. Pre-built wheels are available for Linux (x86_64, aarch64), macOS (x86_64, arm64), and Windows (x64). No Rust toolchain needed.

## Quick Start

```python
import ipextract

# Extract all IPs from a string
ips = ipextract.extract("Connection from 192.168.1.1 and 8.8.8.8")
# ["192.168.1.1", "8.8.8.8"]

# Deduplicate, preserving first-seen order
ips = ipextract.extract_unique("1.1.1.1 8.8.8.8 1.1.1.1")
# ["1.1.1.1", "8.8.8.8"]

# bytes input works too (useful when reading log files directly)
ips = ipextract.extract(b"host 10.0.0.1 connected")
# ["10.0.0.1"]
```

## Reusable Extractor

For processing many lines (log analysis, batch jobs), create an `Extractor` once and reuse it. This avoids redundant initialization on each call.

```python
extractor = ipextract.Extractor().only_public()

for line in log_file:
    ips = extractor.extract(line)
    if ips:
        process(ips)
```

## Filtering

By default, all IP addresses are extracted — IPv4, IPv6, private ranges, loopback, and broadcast. Use the builder methods to filter:

```python
# Only publicly routable IPs (excludes RFC 1918, loopback, broadcast)
e = ipextract.Extractor().only_public()

# Specific exclusions
e = ipextract.Extractor().ignore_private().ignore_loopback()

# Constructor kwargs for one-shot config
e = ipextract.Extractor(private=False, loopback=False, ipv6=False)
```

Fluent methods return a **new** `Extractor` — the original is not modified, making partial configs safe to reuse:

```python
base = ipextract.Extractor()
public = base.only_public()   # new object
ipv4_only = base.ipv6(False)  # new object, base unchanged
```

### Filter reference

| Method | Effect |
|--------|--------|
| `.only_public()` | Exclude private, loopback, and broadcast |
| `.ignore_private()` | Exclude RFC 1918 (IPv4) and ULA/link-local (IPv6) |
| `.ignore_loopback()` | Exclude 127.0.0.0/8 and ::1 |
| `.ignore_broadcast()` | Exclude 255.255.255.255 and link-local ranges |
| `.ipv4(False)` | Skip IPv4 entirely |
| `.ipv6(False)` | Skip IPv6 entirely |
| `.private_ips(bool)` | Enable/disable private IPs |
| `.loopback_ips(bool)` | Enable/disable loopback IPs |
| `.broadcast_ips(bool)` | Enable/disable broadcast IPs |

## Extraction Methods

### `extract(text)` → `list[str]`

Returns all IP addresses found, in order of appearance.

```python
ipextract.Extractor().extract("a 1.1.1.1 b 2.2.2.2")
# ["1.1.1.1", "2.2.2.2"]
```

### `extract_unique(text)` → `list[str]`

Returns unique IP addresses, preserving first-seen order.

```python
ipextract.Extractor().extract_unique("1.1.1.1 2.2.2.2 1.1.1.1")
# ["1.1.1.1", "2.2.2.2"]
```

### `extract_with_offsets(text)` → `list[tuple[str, int, int]]`

Returns `(ip, start, end)` tuples. The byte offsets index directly into the original input — useful for annotation, highlighting, or structured log parsing.

```python
text = "host 1.2.3.4 port 80"
for ip, start, end in ipextract.Extractor().extract_with_offsets(text):
    print(f"{ip} at [{start}:{end}]")
    assert text[start:end] == ip
# 1.2.3.4 at [5:12]
```

## Module-Level Convenience Functions

`ipextract.extract()` and `ipextract.extract_unique()` are shorthand for `Extractor().extract()` with default settings (all IPs included). For repeated calls, prefer creating an `Extractor` instance explicitly.

## Source

The `ipextract` Python package lives in [`crates/ipextract-py/`](https://github.com/erichutchins/geoipsed/tree/main/crates/ipextract-py) and wraps the [`ip-extract`](ip-extract.md) Rust crate.
