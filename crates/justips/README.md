# justips

_Blazing fast, standalone IP address extraction written in Rust._

`justips` finds and extracts IPv4 and IPv6 addresses from unstructured text as fast as possible. It is powered by the same compile-time DFA engine as [`geoipsed`](https://github.com/erichutchins/geoipsed) but purpose-built for raw extraction — a faster, validating alternative to `grep -o`.

## Installation

```bash
cargo install justips
```

## Performance

Benchmarked against a 1.7GB Suricata log dataset (15.4M lines, 30.7M IPs):

| Mode | Time | Overhead |
| :--- | :---: | :---: |
| **Stream** (default) | **857ms** | — |
| **Unique unordered** (`-u`) | **925ms** | +8% |
| **Unique ordered** (`-U`) | **967ms** | +13% |

Also:
- **2x faster** than `geoipsed -j` (parallel mmap vs single-threaded)
- **3x faster** than `ripgrep` (`rg -o`) for extraction

## Usage

```bash
# Extract all IPs from a file
justips access.log

# Unique IPs, unordered (fastest dedup)
justips -u access.log

# Unique IPs, preserving first-seen order (like awk '!seen[$0]++')
justips -U access.log

# Extract from stdin
tail -f access.log | justips

# Filter for only routable IPs
justips --no-private --no-loopback --no-broadcast network.txt
```

### Options

| Flag | Description |
| :--- | :--- |
| `-u`, `--unique` | Deduplicate IPs (unordered, fastest) |
| `-U`, `--unique-ordered` | Deduplicate IPs, preserving first-seen order |
| `--all` | Include all IPs (private, loopback, etc) |
| `--no-private` | Exclude RFC 1918 and ULA ranges |
| `--no-loopback` | Exclude 127.0.0.1 and ::1 |
| `--no-broadcast` | Exclude broadcast and link-local ranges |

## Why use this instead of ripgrep?

1. **Validation**: `ripgrep` matches raw text patterns. `justips` validates every result as a real IP address (e.g., skipping `999.999.999.999`).
2. **Specialized DFA**: Our IP-specific DFA is faster than general-purpose regex engines for this specific needle.
3. **Defang Support**: Automatically handles and normalizes defanged IPs (e.g., `192[.]168[.]1[.]1`).
4. **Built-in dedup**: Two modes — `-u` for fastest unordered unique, `-U` for first-seen order. Both add minimal overhead (~8-13%).

## License

MIT OR Unlicense
