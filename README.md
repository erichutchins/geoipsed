# geoipsed

*Fast, inline geolocation decoration of IPv4 and IPv6 addresses written in Rust*

## Concept

IP address metadata is essential to network defense and incident response. City and country-level geolocation can be clues to rule in or rule out IPs of interest. ASN metadata provides insights of network ownership as well as simply identifying which IPs are internet-routable vs which are internal, private, or bogons.

In command line log analysis, IP geolocation metadata is most useful in *addition* to the already available log metadata on each line. `geoipsed` enriches IP addresses *in place* leaving the existing context intact.

## Architecture

`geoipsed` uses an optimized regex-automata based approach for finding IP addresses in text, with validation to ensure only actual IP addresses are matched. It implements a builder pattern for configuring the IP extraction behavior and a tagging system for handling the decorations.

Key components:
- `Extractor`: Searches text for IP addresses using optimized regex patterns
- `GeoIPSed`: Handles IP geolocation using MaxMind databases
- `Tag`: Represents an IP address with its decoration and position in text
- `Tagged`: Manages a collection of tags within text, with options for inline decoration or JSON output

## Features
* IPv4 and IPv6 address support
* City, Country, ASN, time zone fields among the available metadata
* Flexible templating to customize how `geoipsed` decorates matching IPs
* Coloring to more readily spot the IPs in the logs
* Optional mode to just emit the matching IPs just like grep/ripgrep `-o` parameter
* JSON output mode to extract IPs with their positions in the text (similar to biff's tag feature)
* Line-by-line or whole-file processing options
* Spaces are removed from decoration labels so as not to mess up column numbering in your logs

## Install

`geoipsed` can use various MMDB database formats including:

- **MaxMind** (default): GeoLite2-ASN.mmdb and GeoLite2-City.mmdb files ([download instructions](https://dev.maxmind.com/geoip/updating-databases))
- **IP2Location**: IP2LOCATION-LITE-DB11.MMDB ([download here](https://lite.ip2location.com/))
- **IPinfo**: ip_country_asn.mmdb ([available here](https://ipinfo.io/))

Database locations are searched in this order:
- `/usr/share/GeoIP/` (Linux default)
- `/opt/homebrew/var/GeoIP/` (macOS Homebrew location)
- `/var/lib/GeoIP/` (alternative Linux location)

You can override the location with `-I` or set the environment variable `GEOIP_MMDB_DIR`.

Build and install with cargo:

```
cargo install geoipsed
```

## Usage

```
geoipsed 0.1.3
Inline decoration of IPv4 and IPv6 address geolocations

USAGE:
    geoipsed [OPTIONS] [FILE]...

ARGS:
    <FILE>...    Input file(s) to process. Leave empty or use "-" to read from stdin

OPTIONS:
    -C, --color <COLOR>          Use markers to highlight the matching strings [default: auto]
                                 [possible values: always, never, auto]
    -h, --help                   Print help information
    -I <DIR>                     Specify directory containing the MMDB database files
                                 [env: GEOIP_MMDB_DIR=]
    -L, --list-templates         Display a list of available template substitution parameters to use
                                 in --template format string
    -o, --only-matching          Show only nonempty parts of lines that match
    -t, --template <TEMPLATE>    Specify the format of the IP address decoration. Use the
                                 --list-templates option to see which fields are available. Field
                                 names are enclosed in {}, for example "{field1} any fixed string
                                 {field2} & {field3}"
    --all                        Include all types of IP addresses in matches
    --no-private                 Exclude private IP addresses from matches
    --no-loopback                Exclude loopback IP addresses from matches
    --no-broadcast               Exclude broadcast/link-local IP addresses from matches
    --only-routable              Only include internet-routable IP addresses (requires valid ASN entry)
    --provider <PROVIDER>        Specify the MMDB provider to use (default: maxmind)
    --list-providers             List available MMDB providers and their required files
    --tag                        Output matches as JSON with tag information for each line
    --tag-files                  Output matches as JSON with tag information for entire files
    -V, --version                Print version information
```

Available fields to use in customizing the `-t` parameter:

```
; geoipsed -L
Available template geoip field names are:
{ip}
{asnnum}
{asnorg}
{city}
{continent}
{country_iso}
{country_full}
{latitude}
{longitude}
{timezone}
```

## Examples

### Default mode (in-place IP decoration)
```
$ echo "Connection from 81.2.69.205 to 175.16.199.37" | geoipsed
Connection from <81.2.69.205|AS0_|GB|London> to <175.16.199.37|AS0_|CN|Changchun>
```

### Only matching mode
```
$ echo "Connection from 81.2.69.205 to 175.16.199.37" | geoipsed -o
<81.2.69.205|AS0_|GB|London>
<175.16.199.37|AS0_|CN|Changchun>
```

### Tag mode (JSON output with line-by-line processing)
```
$ echo "Connection from 81.2.69.205 to 175.16.199.37" | geoipsed --tag
{"tags":[{"value":"81.2.69.205","range":[14,25]},{"value":"175.16.199.37","range":[29,42]}],"data":{"text":"Connection from 81.2.69.205 to 175.16.199.37\n"}}
```

### Tag files mode (JSON output with whole-file processing)
```
$ geoipsed --tag-files example.log
{"tags":[{"value":"81.2.69.205","range":[14,25]},{"value":"175.16.199.37","range":[146,159]}],"data":{"text":"Connection from 81.2.69.205 to server\nDetails: request received\nConnection from 175.16.199.37 to server\n"}}
```

## Benchmark
Comparing the Rust implementation to a basic Python version against 30,000 lines (~23MB decompressed) of Suricata json eve logs:

```
Benchmark 1: zstdcat ../30k.log.zst | target/release/geoipsed
  Time (mean ± σ):     157.1 ms ±  16.7 ms    [User: 167.8 ms, System: 28.9 ms]
  Range (min … max):   137.1 ms … 209.9 ms    21 runs
 
Benchmark 2: zstdcat ../30k.log.zst | python python/geoipsed.py
  Time (mean ± σ):     15.209 s ±  0.929 s    [User: 15.213 s, System: 0.210 s]
  Range (min … max):   14.312 s … 17.076 s    10 runs
 
Summary
  'zstdcat ../30k.log.zst | target/release/geoipsed' ran
   96.80 ± 11.84 times faster than 'zstdcat ../30k.log.zst | python python/geoipsed.py'
```

* *Note* that a significant factor of this speed difference is the regular expression matching, specifically the IPv6 pattern. If you match just on IPv4, it was only ~5x faster.

The performance is now further improved by:
- Using `regex-automata` with optimized regex compilation
- Implementing efficient input handling with minimal allocations
- Employing a tagging system to process matches more effectively

## Background & Gratitude
* The historical inspiration for geoipsed was when a sensei taught me long ago that perl's `s/find/replace/g` sed interface can interpret the replace pattern as a perl expression! For example, decoding hexascii, xor'ing, and printing the characters: `perl -pe 's/%([a-f0-9]{2})/chr(hex($1)^0x5e)/ieg`. If I can run perl chr and hex functions, can I import geoip libraries, and run those functions? It turns out, yes! Thus geoipsed was born.

* Writing this was an endeavor to learn more Rust, and I'm grateful for [BurntSushi](https://github.com/BurntSushi) and [sstadick](https://github.com/sstadick) whose awesome crates and well documented code are sources of inspiration and regular references. The architecture was specifically influenced by BurntSushi's [biff](https://github.com/BurntSushi/biff) tool, which demonstrates excellent patterns for text processing and command-line utilities.

* The current version has been refactored to adopt superior design patterns from these tools, including:
  - Using `anyhow` for comprehensive error handling
  - Implementing the extractor/builder pattern for IP address matching
  - Adopting efficient I/O patterns for handling files and stdin
  - Proper validation to eliminate false positives in IP address matching
  - JSON tagging functionality inspired by biff's tag commands
