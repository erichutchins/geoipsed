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
- ~97x faster than Python implementations

## Databases

Supports MaxMind (default), IP2Location, and IPinfo MMDB formats. Specify location with `-I` or `GEOIP_MMDB_DIR` environment variable.

## Usage

```
geoipsed [OPTIONS] [FILE]...

OPTIONS:
  -o, --only-matching          Show only matching IPs
  -t, --template <TEMPLATE>    Custom decoration format
  -C, --color <COLOR>          Color output (always|never|auto)
  -I <DIR>                     Database directory
  -L, --list-templates         Show available fields
  --tag                        JSON output (line-by-line)
  --tag-files                  JSON output (whole files)
  --all                        Include all IP types (default)
  --no-private                 Exclude private IPs
  --no-loopback                Exclude loopback IPs
  --no-broadcast               Exclude broadcast IPs
  --provider <PROVIDER>        MMDB provider (default: maxmind)
  --list-providers             List available providers
```

## Examples

```bash
# Decoration mode
geoipsed access.log

# Only matching IPs
geoipsed -o access.log

# JSON output
geoipsed --tag access.log

# Custom template
geoipsed -t "{ip} in {country_iso}" access.log

# Filter: public IPs only
geoipsed --no-private --no-loopback --no-broadcast access.log
```

## Performance

Processes ~100K lines (~23MB) in **157ms** vs **15s** for equivalent Python implementation (97x speedup).

## Documentation

Full documentation, architecture details, and benchmarks available at [GitHub Pages](https://erichutchins.github.io/geoipsed/).

## Contributing

See [CLAUDE.md](CLAUDE.md) for project conventions and coding patterns.
