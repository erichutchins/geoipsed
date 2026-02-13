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
- ~60x faster than Python implementations (benchmarked with hyperfine)

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
  -j, --justips              Extract IPs only without MMDB lookups or templating (fast path, implies --only-matching)
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

# Literally just the matching IPs
geoipsed -j access.log

# Custom template
geoipsed -t "{ip} in {country_iso}" access.log

# Filter: public IPs only
geoipsed --no-private --no-loopback --no-broadcast access.log

# Advanced: JSON output of matching ranges with before and after decoration
geoipsed --tag access.log
```

## Performance

Processes 100K lines (3.9MB) in **15.3ms** vs **1.0s** for equivalent Python implementation (65x speedup). Scales to **72x** on larger datasets (500K lines).
