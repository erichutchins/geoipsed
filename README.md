# geoipsed

*Fast, inline geolocation decoration of IPv4 and IPv6 addresses written in Rust*

## Concept

IP address metadata is essential to network defense and incident response. City and country-level geolocation can be clues to rule in or rule out IPs of interest. ASN metadata provides insights of network ownership as well as simply identifying which IPs are internet-routable vs which are internal, private, or bogons.

In command line log analysis, IP geolocation metadata is most useful in *addition* to the already available log metadata on each line. `geoipsed` enriches IP addresses *in place* leaving the existing context intact. 

## Features
* IPv4 and IPv6 address support
* City, Country, ASN, time zone fields among the available metadata
* Flexible templating to customize how `geoipsed` decorates matching IPs
* Coloring to more readily spot the IPs in the logs
* Optional mode to just emit the matching IPs just like grep/ripgrep `-o` parameter
* Spaces are removed from decoration labels so as not to mess up column numbering in your logs

## Install

`geoipsed` uses Maxmind's GeoLite2-ASN.mddb and GeoLite2-City.mddb files. Follow the instructions to obtain these files here: https://dev.maxmind.com/geoip/updating-databases

Currently, `geoipsed` looks exclusively to `/usr/share/GeoIP` for the mmdb files, however you can override this with the `-I` command line option or the environment variable `MAXMIND_MMDB_DIR`.

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

## Background & Gratitude
* The historical inspiration for geoipsed was when a sensei taught me long ago that perl's `s/find/replace/g` sed interface can interpret the replace pattern as a perl expression! For example, decoding hexascii, xor'ing, and printing the characters: `perl -pe 's/%([a-f0-9]{2})/chr(hex($1)^0x5e)/ieg`. If I can run perl chr and hex functions, can I import geoip libraries, and run those functions? It turns out, yes! Thus geoipsed was born.

* Writing this was an endeavor to learn more Rust, and I'm grateful for [burntsushi](https://github.com/burntsushi) and [sstadick](https://github.com/sstadick) whose awesome crates and well documented code are sources of inspiration and regular references.
