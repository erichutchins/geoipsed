# CLAUDE.md - Coding Conventions and Patterns for geoipsed

## General Principles

- **Modularity**: Keep related functionality in separate modules (e.g., extract, geoip, mmdb)
- **Extensibility**: Use traits and builder patterns to allow easy extension
- **Error Handling**: Use `anyhow` consistently with context for all errors
- **Documentation**: Document public APIs and add examples where relevant

## Code Organization

### File Structure

```
geoipsed/
├── src/
│   ├── main.rs       # CLI interface and command orchestration
│   ├── extractor.rs  # IP address extraction logic
│   ├── geoip.rs      # Traditional IP geolocation logic
│   ├── mmdb.rs       # Provider system for MMDB formats
│   ├── tag.rs        # Tag representation and serialization
│   ├── input.rs      # File/stdin handling
│   └── files.rs      # File-specific operations
├── tests/            # Integration tests
│   └── cli.rs        # Command-line integration tests
├── examples/         # Example usage code
└── GUIDE.md          # User documentation
```

### Module Dependencies

- Core functionality: `extractor.rs` and `mmdb.rs` 
- Input handling: `input.rs` and `files.rs`
- Output formatting: `tag.rs`
- CLI interface and config: `main.rs`

## Coding Patterns

### Error Handling

Use the `anyhow` pattern for error handling:

```rust
// Good:
fn do_something() -> anyhow::Result<()> {
    let file = std::fs::File::open("file.txt")
        .with_context(|| format!("Failed to open file"))?;
    Ok(())
}

// Avoid:
fn do_something() -> Result<(), std::io::Error> {
    let file = std::fs::File::open("file.txt")?;
    Ok(())
}
```

### Builder Pattern

Use builder pattern for complex objects:

```rust
let extractor = ExtractorBuilder::new()
    .ipv4(true)
    .ipv6(true)
    .private_ips(include_private)
    .loopback_ips(include_loopback)
    .broadcast_ips(include_broadcast)
    .only_routable(only_routable)
    .all_matches(true)
    .build()?;
```

### Provider Pattern

The `MmdbProvider` trait defines a common interface for different MMDB providers:

```rust
pub trait MmdbProvider: fmt::Debug {
    fn name(&self) -> &str;
    fn default_path(&self) -> PathBuf;
    fn required_files(&self) -> Vec<String>;
    fn available_fields(&self) -> Vec<TemplateField>;
    fn initialize(&mut self, path: &Path) -> Result<()>;
    fn lookup(&self, ip: &str, template: &str) -> Result<String>;
    fn has_asn(&self, ip: &str) -> bool;
}
```

### Tagged Output Pattern

The `Tag` and `Tagged` types allow flexibility in how IP addresses are processed:

```rust
// Create a tagged representation of text with IPs
let mut tagged = Tagged::new(line.full());
for range in extractor.find_iter(haystack) {
    tagged = tagged.tag(Tag::new(ipstr).with_range(range));
}

// Output as JSON or decorated text
if json_mode {
    tagged.write_json(&mut output)?;
} else {
    tagged.write(&mut output)?;
}
```

## Command-Line Interface Conventions

- Use `--flag` format for boolean options
- Use negated flags (e.g., `--no-private`) when disabling is less common
- Use `--all` to enable all subtypes of a feature
- Provide both short (`-o`) and long (`--only-matching`) forms for common options
- Support environment variables for paths (`GEOIP_MMDB_DIR`)

## Adding New MMDB Providers

1. Implement the `MmdbProvider` trait
2. Define the MMDB format and field mappings
3. Register the provider in `ProviderRegistry::default()`
4. Update documentation in GUIDE.md

## Testing Strategies

- Mock MMDB files in tests to avoid external dependencies
- Test both decorating and tagging modes
- Test various input formats (IPv4, IPv6, invalid)
- Test environment variable handling

## Performance Considerations

- Cache IP lookups to avoid redundant database access
- Use efficient regex implementations (regex-automata)
- Minimize allocations in the main processing loop
- Use streaming I/O for large files

## Dependencies

- **maxminddb**: Core MMDB reading functionality
- **anyhow**: Error handling
- **clap**: Command-line argument parsing
- **regex-automata**: Optimized regex pattern matching
- **camino**: UTF-8 path handling
- **termcolor**: Colored terminal output
- **serde/serde_json**: JSON serialization for tag mode

## Helpful Resources

- MaxMind GeoIP2 format: https://maxmind.github.io/MaxMind-DB/
- IP2Location: https://www.ip2location.com/
- IPinfo: https://ipinfo.io/
- https://github.com/sapics/ip-location-db (collection of free IP databases)

## Commit Conventions

- Use imperative mood in commit messages (e.g., "Add IPinfo support" not "Added IPinfo support")
- Include the component being modified (e.g., "extractor: Improve IPv6 validation")
- Reference issues where applicable (e.g., "Fix #23: Handle broken pipes gracefully")