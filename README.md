# geoipsed

_Fast, inline geolocation decoration of IPv4 and IPv6 addresses written in Rust_

IP geolocation enriches logs with City, Country, ASN, and timezone metadata. `geoipsed` finds and decorates IP addresses in-place, leaving existing context intact—perfect for incident response and network analysis.

## Quick Start

```bash
cargo install geoipsed
echo "Connection from 81.2.69.205 to 175.16.199.37" | geoipsed
```

Output:

```
Connection from <81.2.69.205|AS0_|GB|London> to <175.16.199.37|AS0_|CN|Changchun>
```

## Features

- IPv4 and IPv6 support with strict validation
- City, Country, ASN, timezone metadata
- Flexible templating via `-t/--template`
- Inline decoration or JSON output modes (`--tag`, `--tag-files`)
- Fine-grained filtering: `--all`, `--no-private`, `--no-loopback`, `--no-broadcast`
- Color support with `-C/--color`
- Streaming input (stdin or multiple files)
- ~100x faster than Python implementations

## Databases

Supports MaxMind (default), IP2Location, and IPinfo MMDB formats. Specify location with `-I` or `GEOIP_MMDB_DIR` environment variable.

## Usage

```
geoipsed --help
Inline decoration of IPv4 and IPv6 address geolocations

Usage: geoipsed [OPTIONS] [FILE]...

Arguments:
  [FILE]...  Input file(s) to process. Leave empty or use "-" to read from stdin

Options:
  -o, --only-matching        Show only nonempty parts of lines that match
  -C, --color <COLOR>        Use markers to highlight the matching strings [default: auto] [possible values: always, never, auto]
  -t, --template <TEMPLATE>  Specify the format of the IP address decoration. Use the --list-templates option to see which fields are available. Field names are enclosed in {}, for example "{field1} any fixed string {field2} & {field3}"
      --tag                  Output matches as JSON with tag information for each line
      --tag-files            Output matches as JSON with tag information for entire files
      --all                  Include all types of IP addresses in matches
      --no-private           Exclude private IP addresses from matches
      --no-loopback          Exclude loopback IP addresses from matches
      --no-broadcast         Exclude broadcast/link-local IP addresses from matches
      --only-routable        Only include internet-routable IP addresses (requires valid ASN entry)
      --provider <PROVIDER>  Specify the MMDB provider to use (default: maxmind) [default: maxmind]
  -I <DIR>                   Specify directory containing the MMDB database files [env: GEOIP_MMDB_DIR=]
      --list-providers       List available MMDB providers and their required files
  -L, --list-templates       Display a list of available template substitution parameters to use in --template format string
  -h, --help                 Print help
  -V, --version              Print version
```

## Examples

```bash
# Decoration mode
geoipsed access.log

# Only matching IPs (with decoration)
geoipsed -o access.log

# Custom template
geoipsed -t "{ip} in {country_iso}" access.log

# Filter: public IPs only
geoipsed --no-private --no-loopback --no-broadcast access.log

# Advanced: JSON output of matching ranges with before and after decoration
geoipsed --tag access.log
```

### Extracting just IPs

For scenarios where you only need a raw list of IP addresses (like `grep -o` but faster and with IP validation), use the standalone `justips` tool:

```bash
cargo install justips
justips access.log
```

`justips` is a specialized, zero-dependency version of the extraction engine that is ~45% faster than `ripgrep` for finding IPs.

## Performance

`geoipsed` is highly optimized for sequential IP extraction, even outperforming `ripgrep` itself for this specific task.

Benchmarked against a **1.7GB Suricata log** (15.4M lines, 30.7M IP matches):

| Tool | Mode | Time | Throughput | Speedup |
| :--- | :--- | :---: | :---: | :---: |
| **`justips`** | **Parallel mmap + DFA** | **857ms** | **~2 GiB/s** | **7.2x** |
| `ripgrep` | `rg -ao` (v4/v6 regex) | 6.17s | ~275 MiB/s | Baseline |
| Python (`re`) | `IPRE.sub()` (baseline) | 431s | ~4 MiB/s | 0.01x |

For raw IP extraction (no geolocation), use the standalone [`justips`](crates/justips/) tool — it uses parallel mmap processing and is purpose-built for maximum throughput.

**Why is the DFA so fast?** While `ripgrep` is a world-class general search tool, `geoipsed` and `justips` use a specialized, compile-time DFA generated via `regex-automata`. This allows parsing and validating every `IpAddr` during the scan faster than a general regex engine can match the raw text.

## Workspace Crates

| Crate | Description |
| :--- | :--- |
| [`ip-extract`](crates/ip-extract/) | Zero-copy IP extraction library — compile-time DFA, defang support, builder pattern |
| [`justips`](crates/justips/) | Standalone CLI for fast IP extraction — parallel mmap, built-in dedup (`-u`, `-U`) |
| [`ipextract`](crates/ipextract-py/) | Python bindings (PyO3 + maturin) — stable ABI, published to [PyPI](https://pypi.org/project/ipextract/) |

## Documentation

Full documentation, architecture details, and benchmarks available at [GitHub Pages](https://erichutchins.github.io/geoipsed/).

## Contributing

See [CLAUDE.md](CLAUDE.md) for project conventions and coding patterns.
