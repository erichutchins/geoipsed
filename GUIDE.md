# geoipsed - User Guide

## Table of Contents

1. [Introduction](#introduction)
2. [MMDB Format and Providers](#mmdb-format-and-providers)
3. [Installation and Configuration](#installation-and-configuration)
4. [Using geoipsed](#using-geoipsed)
5. [MMDB Providers](#mmdb-providers)
6. [Adding Custom MMDB Formats](#adding-custom-mmdb-formats)
7. [Filtering Options](#filtering-options)
8. [Template Reference](#template-reference)

## Introduction

`geoipsed` is a tool for inline decoration of IPv4 and IPv6 addresses with geolocation information. It scans text input for IP addresses and enriches them with metadata such as country, city, ASN, and more.

This guide covers:
- How to use different MMDB (MaxMind Database) providers
- How to configure and customize IP address decoration
- How to extend geoipsed with custom MMDB formats

## MMDB Format and Providers

### What Are MMDB Files?

MMDB (MaxMind Database) files are binary databases designed for high-performance IP geolocation lookups. These binary files store IP address ranges along with associated metadata like country, city, organization, latitude/longitude, and more.

Several organizations provide MMDB files with varying data quality, coverage, and licensing terms:

1. **MaxMind** - The creator of the MMDB format, offers both free GeoLite2 databases and commercial GeoIP2 databases
2. **IP2Location** - Offers commercial IP geolocation databases in MMDB format
3. **IPinfo** - Provides IP data with various attributes in MMDB format
4. **DB-IP** - Offers IP to geolocation databases in multiple formats including MMDB

By default, geoipsed is configured to work with MaxMind's GeoLite2 databases, but can be extended to work with other providers.

## Installation and Configuration

### MaxMind GeoLite2 (Default)

1. Create a MaxMind account at [dev.maxmind.com](https://dev.maxmind.com/geoip/geolite2-free-geolocation-data)
2. Download the GeoLite2 databases:
   - GeoLite2-ASN.mmdb
   - GeoLite2-City.mmdb
   - Alternatively, you can use the IPv4/IPv6 specific versions
3. Place the files in one of these directories (checked in this order):
   - `/usr/share/GeoIP/` (Linux default)
   - `/opt/homebrew/var/GeoIP/` (macOS Homebrew location)
   - `/var/lib/GeoIP/` (alternative Linux location)
   -   Or use a custom directory with the `-I` option or `GEOIP_MMDB_DIR` environment variable

```bash
# Using default location
sudo mkdir -p /usr/share/GeoIP
sudo cp GeoLite2-ASN.mmdb GeoLite2-City.mmdb /usr/share/GeoIP/

# Using custom location
mkdir -p ~/geoip
cp GeoLite2-ASN.mmdb GeoLite2-City.mmdb ~/geoip/
geoipsed -I ~/geoip ...

# Setting via environment variable
export GEOIP_MMDB_DIR=~/geoip
geoipsed ...
```

### IP2Location

1. Create an account at [lite.ip2location.com](https://lite.ip2location.com/)
2. Download the IP2Location LITE databases in MMDB format
3. Place the files in `/usr/share/IP2Location/` (default) or a custom directory

```bash
# Using default location
sudo mkdir -p /usr/share/IP2Location
sudo cp IP2LOCATION-LITE-DB11.MMDB /usr/share/IP2Location/

# Using with geoipsed
geoipsed --provider ip2location -I /usr/share/IP2Location ...
```

### IPinfo

1. Create an account at [ipinfo.io](https://ipinfo.io/)
2. Download the IPinfo database in MMDB format (or use the sample available at [GitHub](https://github.com/ipinfo/sample-database/blob/main/IP%20to%20Country%20ASN/ip_country_asn_sample.mmdb))
3. Place the files in `/usr/share/IPinfo/` (default) or a custom directory

```bash
# Using default location
sudo mkdir -p /usr/share/IPinfo
sudo cp ip_country_asn.mmdb /usr/share/IPinfo/

# Using with geoipsed
geoipsed --provider ipinfo -I /usr/share/IPinfo ...
```

### Other Providers

For other providers, follow a similar process:
1. Download the MMDB files
2. Place them in an appropriate directory
3. Use the `--provider` and `-I` options to specify the provider and location

## Using geoipsed

### Basic Usage

```bash
# Default mode (MaxMind GeoLite2)
cat logfile.txt | geoipsed

# Specify MMDB provider
cat logfile.txt | geoipsed --provider ip2location

# Specify custom MMDB directory
cat logfile.txt | geoipsed -I /path/to/mmdb/files

# Custom template for decoration
cat logfile.txt | geoipsed -t "IP:{ip} (Country:{country_iso})"

# Only show matching IPs
cat logfile.txt | geoipsed -o
```

### List Available Providers

```bash
geoipsed --list-providers
```

This will show all available providers, their default paths, required files, and their installation status with colored indicators (✓ for installed, ✗ for not installed). It also displays the status of your `GEOIP_MMDB_DIR` environment variable.

### List Available Template Fields

```bash
geoipsed --list-templates
geoipsed --provider ip2location --list-templates
```

## MMDB Providers

geoipsed includes support for the following providers out of the box:

1. **maxmind** (default) - Uses MaxMind GeoLite2/GeoIP2 databases
   - Required files: GeoLite2-ASN.mmdb, GeoLite2-City.mmdb, or their IPv4/IPv6 specific variants
   - Default paths (checked in this order):
     - /usr/share/GeoIP/
     - /opt/homebrew/var/GeoIP/ (macOS Homebrew location)
     - /var/lib/GeoIP/

2. **ip2location** - Uses IP2Location databases
   - Required files: IP2LOCATION-LITE-DB11.MMDB
   - Default path: /usr/share/IP2Location/

3. **ipinfo** - Uses IPinfo databases with combined ASN and location data
   - Required files: ip_country_asn.mmdb
   - Default path: /usr/share/IPinfo/
   - Supports looking up both ASN and location information from a single file

## Adding Custom MMDB Formats

geoipsed can be extended to support custom or proprietary MMDB formats. There are several ways to do this:

### Method 1: Use the define_mmdb_provider! Macro

For simple cases, you can create a new provider module with the `define_mmdb_provider!` macro:

```rust
// In your_project.rs or any Rust file that uses geoipsed as a library

use geoipsed::{
    ProviderRegistry, 
    TemplateField, 
    define_mmdb_provider,
    register_mmdb_provider
};

pub fn register_my_provider(registry: &mut ProviderRegistry) {
    let provider = define_mmdb_provider!(
        name: "My Custom Provider",
        default_path: "/path/to/custom/mmdb",
        files: ["custom.mmdb"],
        fields: [
            "ip" => {
                description: "The IP address itself",
                example: "93.184.216.34"
            },
            "custom_field" => {
                description: "My custom field",
                example: "Some Value"
            }
        ]
    );
    
    register_mmdb_provider!(registry, "mycustom", provider);
}

fn main() {
    // Create a provider registry
    let mut registry = ProviderRegistry::default();
    
    // Register your custom provider
    register_my_provider(&mut registry);
    
    // Use the registry with geoipsed functions
    // ...
}
```

The library's public API makes it easy to extend geoipsed with your own providers.

### Method 2: Implement the MmdbProvider Trait

For more complex scenarios, implement the `MmdbProvider` trait directly:

```rust
// In your_project.rs or any Rust file that uses geoipsed as a library

use std::path::{Path, PathBuf};
use std::fmt;
use anyhow::Result;
use maxminddb::{Mmap, Reader};
use geoipsed::{MmdbProvider, TemplateField, ProviderRegistry};

#[derive(Debug)]
pub struct CustomProvider {
    name: String,
    initialized: bool,
    reader: Option<Reader<Mmap>>,
    // Add any custom fields needed
}

impl Default for CustomProvider {
    fn default() -> Self {
        Self {
            name: "Custom Provider".to_string(),
            initialized: false,
            reader: None,
        }
    }
}

impl MmdbProvider for CustomProvider {
    fn name(&self) -> &str {
        &self.name
    }
    
    fn default_path(&self) -> PathBuf {
        PathBuf::from("/path/to/custom/mmdb")
    }
    
    fn required_files(&self) -> Vec<String> {
        vec!["custom.mmdb".to_string()]
    }
    
    fn available_fields(&self) -> Vec<TemplateField> {
        vec![
            TemplateField {
                name: "ip".to_string(),
                description: "IP address".to_string(),
                example: "93.184.216.34".to_string(),
            },
            // Add more fields
        ]
    }
    
    fn initialize(&mut self, path: &Path) -> Result<()> {
        // Implementation details - open the .mmdb file
        let db_path = path.join("custom.mmdb");
        if db_path.exists() {
            self.reader = Some(
                Reader::open_mmap(&db_path)
                    .with_context(|| format!("Failed to open database at {}", db_path.display()))?
            );
            self.initialized = true;
        }
        Ok(())
    }
    
    fn lookup(&self, ip_str: &str, template: &str) -> Result<String> {
        // Your custom lookup implementation
        // Use self.reader to access the MMDB and extract fields
        Ok(template.replace("{ip}", ip_str))
    }
    
    fn has_asn(&self, _ip_str: &str) -> bool {
        // Determine if the IP has ASN information
        false
    }
}

fn main() {
    // Create a provider registry
    let mut registry = ProviderRegistry::default();
    
    // Register your custom provider
    registry.register("custom".to_string(), Box::new(CustomProvider::default()));
    
    // Use the registry with geoipsed functions
    // ...
}
```

This approach gives you complete control over how IP data is loaded and processed, allowing for custom MMDB formats or even entirely different database formats.

### Method 3: Contributing to geoipsed

For widely-used MMDB formats, consider contributing to the geoipsed project:

1. Fork the repository
2. Add your provider implementation in `src/mmdb.rs` or a new module
3. Register it in the `ProviderRegistry::default()` implementation
4. Submit a pull request

## Filtering Options

geoipsed provides several options to filter which IP addresses are matched and decorated:

```bash
# Include all types of IP addresses (private, loopback, broadcast)
geoipsed --all

# Exclude private IP addresses (10.0.0.0/8, 192.168.0.0/16, etc.)
geoipsed --no-private

# Exclude loopback IP addresses (127.0.0.0/8, ::1)
geoipsed --no-loopback

# Exclude broadcast or link-local IP addresses (255.255.255.255, 169.254.x.x)
geoipsed --no-broadcast

# Only include internet-routable IPs (those with ASN information)
geoipsed --only-routable
```

By default, geoipsed excludes special-purpose IP addresses, showing only "normal" addresses. Use the `--all` flag to include all IP types, or selectively disable specific categories with the `--no-*` flags.

## Benchmarking

geoipsed includes a benchmark script to compare performance with previous versions:

```bash
# Run the benchmark
./benchmark.sh
```

This uses hyperfine for accurate measurements and compares:
- The installed version (from crates.io)
- The newly built version

Performance improvements may vary based on the specific use case, but the design patterns introduced should provide better extensibility and maintainability without sacrificing speed.

### Template Reference

### MaxMind Provider Fields

| Field | Description | Example |
|-------|-------------|---------|
| `{ip}` | The IP address itself | 93.184.216.34 |
| `{asnnum}` | Autonomous System Number | 15133 |
| `{asnorg}` | Autonomous System Organization | MCI Communications Services |
| `{city}` | City name | Los Angeles |
| `{continent}` | Continent code | NA |
| `{country_iso}` | Country ISO code | US |
| `{country_full}` | Full country name | United States |
| `{latitude}` | Latitude coordinate | 34.0544 |
| `{longitude}` | Longitude coordinate | -118.2441 |
| `{timezone}` | Time zone name | America/Los_Angeles |

### IP2Location Provider Fields

| Field | Description | Example |
|-------|-------------|---------|
| `{ip}` | The IP address itself | 93.184.216.34 |
| `{country_code}` | Country code | US |
| `{country_name}` | Country name | United States of America |
| `{region}` | Region/State name | California |
| `{city}` | City name | Los Angeles |
| `{latitude}` | Latitude coordinate | 34.0544 |
| `{longitude}` | Longitude coordinate | -118.2441 |

### IPinfo Provider Fields

| Field | Description | Example |
|-------|-------------|---------|
| `{ip}` | The IP address itself | 93.184.216.34 |
| `{country}` | Country code | US |
| `{country_name}` | Country name | United States |
| `{asn}` | ASN with AS prefix | AS15133 |
| `{asn_name}` | ASN organization name | MCI Communications Services |

For the most current list of available fields for each provider, use the `--list-templates` option:

```bash
geoipsed --provider maxmind --list-templates
geoipsed --provider ip2location --list-templates
```