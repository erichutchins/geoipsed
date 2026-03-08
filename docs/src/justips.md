# justips

Blazing fast, standalone IP address extraction.

`justips` finds and extracts IPv4 and IPv6 addresses from unstructured text as fast as possible. It is powered by the same compile-time DFA engine as `geoipsed` but purpose-built for raw extraction — a faster, validating alternative to `grep -o`.

## Installation

```bash
cargo install justips
```

## Usage

```bash
# Extract all IPs from a file
justips access.log

# Unique IPs, unordered (fastest dedup)
justips -u access.log

# Unique IPs, preserving first-seen order
justips -U access.log

# Extract from stdin
tail -f access.log | justips

# Filter for only routable IPs
justips --no-private --no-loopback --no-broadcast network.txt

# Multiple files
justips access.log error.log firewall.log
```

## Options

| Flag | Description |
| :--- | :--- |
| `-u`, `--unique` | Deduplicate IPs (unordered, fastest) |
| `-U`, `--unique-ordered` | Deduplicate IPs, preserving first-seen order |
| `--all` | Include all IPs (private, loopback, etc) |
| `--no-private` | Exclude RFC 1918 and ULA ranges |
| `--no-loopback` | Exclude 127.0.0.1 and ::1 |
| `--no-broadcast` | Exclude broadcast and link-local ranges |

## Deduplication Modes

### No dedup (default)

Streams IPs directly to stdout as they are found. Zero memory overhead — output begins immediately.

### `-u` / `--unique` (unordered)

Hash-based dedup using `HashSet`. Each rayon chunk builds its own `HashSet<String>`, then all sets are merged. Output order is not guaranteed.

Best for: feeding into other tools where order doesn't matter (e.g., enrichment pipelines, blocklist generation).

### `-U` / `--unique-ordered` (first-seen order)

Order-preserving dedup using `IndexSet`. Each rayon chunk builds its own `IndexSet<String>`, then sets are merged in chunk order. The first occurrence of each IP determines its position in the output.

Best for: preserving chronological context (e.g., "which IPs appeared first in these logs?").

## Performance

Benchmarked against a 1.7GB Suricata log dataset (15.4M lines, 30.7M IPs):

| Mode | Time | Overhead |
| :--- | :---: | :---: |
| **Stream** (default) | **857ms** | — |
| **Unique unordered** (`-u`) | **925ms** | +8% |
| **Unique ordered** (`-U`) | **967ms** | +13% |

### Architecture

- **Files**: Memory-mapped (`mmap`) and split into ~4MB chunks at newline boundaries, processed in parallel with rayon
- **stdin**: Line-buffered streaming via `ripline` (single-threaded, suitable for pipes)
- **Defang**: Automatically recognizes and normalizes defanged IPs (`192[.]168[.]1[.]1` → `192.168.1.1`)

## When to use justips vs geoipsed

| Need | Tool |
| :--- | :--- |
| Raw list of IPs | `justips` |
| Unique IPs from logs | `justips -u` or `justips -U` |
| IPs with geolocation metadata | `geoipsed` |
| Inline decoration of log lines | `geoipsed` |
| JSON output with IP positions | `geoipsed --tag` |
