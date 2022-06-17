# geoipsed

*Fast, inline geolocation decoration of IPv4 and IPv6 addresses written in Rust*

## Concept

IP address metadata is essential to network defense and incident response. City and country-level geolocation can be clues to rule in or rule out IPs of interest. ASN metadata provides insights of network ownership as well as simply identifying which IPs are internet-routable vs which are internal, private, or bogons.

In command line log analysis, as useful as IP geolocation metadata can be alone, it's most useful in *addition* to the already available log metadata. `geoipsed` enriches IP addresses *in place* leaving the existing context intact. 

## Features
* IPv4 and IPv6 address support
* City, Country, Country ISO Code, ASN, and time zone metadata available
* Flexible templating language to customize how `geoipsed` decorates matching IPs
* Coloring to more readily spot the IPs in the logs
* Optional mode to just emit the matching IPs just like grep/ripgrep `-o` parameter
* Spaces are removed from decoration labels so as not to mess up column delimiters

## Install

`geoipsed` uses Maxmind's GeoLite2-ASN.mddb and GeoLite2-City.mddb files. Follow the instructions to obtain these files here: https://dev.maxmind.com/geoip/updating-databases

## Usage

```
geoipsed 0.1.0
Inline decoration of IPv4 and IPv6 address geolocations

USAGE:
    geoipsed [OPTIONS] [FILE]...

ARGS:
    <FILE>...    Input file(s) to process. Leave empty or use "-" to read from stdin

OPTIONS:
    -C, --color <COLOR>          Use markers to highlight the matching strings [default: auto]
                                 [possible values: always, never, auto]
    -h, --help                   Print help information
    -I <DIR>                     Specify directory containing GeoLite2-ASN.mmdb and
                                 GeoLite2-City.mmdb [env: MAXMIND_MMDB_DIR=]
    -L, --list-templates         Display a list of available template substitution parameters to use
                                 in --template format string
    -o, --only-matching          Show only nonempty parts of lines that match
    -t, --template <TEMPLATE>    Specify the format of the IP address decoration. Use the
                                 --list-templates option to see which fields are available. Field
                                 names are enclosed in {}, for example "{field1} any fixed string
                                 {field2} & {field3}"
    -V, --version                Print version information
```

## TODO

## Design Notes

