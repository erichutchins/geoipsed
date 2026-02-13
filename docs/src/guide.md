# User Guide

This guide covers the usage and configuration of `geoipsed`.

## Installation

```bash
cargo install geoipsed
```

## Basic Usage

The simplest way to use `geoipsed` is to pipe text into it:

```bash
echo "8.8.8.8" | geoipsed
```

## Configuration

### MMDB Databases

`geoipsed` requires MMDB files to perform geolocations. You can specify the directory containing these files using the `-I` flag or the `GEOIP_MMDB_DIR` environment variable.

### Templates

You can customize the output using the `-t/--template` flag. Use `{field}` placeholders for metadata.

Example:
```bash
geoipsed -t "{ip} is in {country_name}"
```

Use `geoipsed --list-templates` to see all available fields.
