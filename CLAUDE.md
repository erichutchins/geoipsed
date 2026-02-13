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
│   ├── lib.rs        # Library exports (public API)
│   ├── geoip.rs      # GeoIP lookup and decoration logic
│   ├── mmdb.rs       # Provider system for MMDB formats
│   ├── template.rs   # Template field substitution engine
│   ├── tag.rs        # Tag representation and JSON serialization
│   ├── input.rs      # File/stdin handling
│   ├── files.rs      # File-specific operations (tag-files mode)
│   ├── error.rs      # Error types and context
│   └── tests/        # Integration tests
├── crates/ip-extract/  # IP extraction crate (DFA-based, zero-copy)
│   ├── src/lib.rs      # Extractor with builder pattern
│   ├── build.rs        # Compile-time DFA generation
│   └── tests/          # Extraction tests
├── python/           # Python reference implementation
│   └── geoipsed.py   # Equivalent tool for benchmarking
└── tests/
    └── maxmind/      # MMDB test fixtures
```

### Module Dependencies

- **IP Extraction**: `ip-extract` crate (separate binary crate, DFA-based zero-copy)
- **Core functionality**: `mmdb.rs` (provider registry), `geoip.rs` (lookup/decoration)
- **Output formatting**: `tag.rs` (JSON tags), `template.rs` (field substitution)
- **Input/Output**: `input.rs` (file/stdin), `files.rs` (tag-files mode)
- **CLI interface**: `main.rs` (orchestration, streaming loops)

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

Use builder pattern for IP extraction (from ip-extract crate):

```rust
// Extract all IPs (default includes private, loopback, broadcast)
let extractor = ExtractorBuilder::new().build()?;

// Extract only public IPs
let extractor = ExtractorBuilder::new().only_public().build()?;

// Granular filtering with chaining
let mut builder = ExtractorBuilder::new();
if !include_private {
    builder.ignore_private();
}
if !include_loopback {
    builder.ignore_loopback();
}
if !include_broadcast {
    builder.ignore_broadcast();
}
let extractor = builder.build()?;
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
- Use opt-out negated flags (e.g., `--no-private`) to disable features (principle of least surprise)
- Use `--all` to include all IP types (private, loopback, broadcast)
- Provide both short (`-o`, `-C`, `-t`, `-L`, `-I`) and long forms for common options
- Support environment variables for paths (`GEOIP_MMDB_DIR`)
- New options: `-j/--justips` for fast IP-extraction-only mode (no MMDB lookups)

## Adding New MMDB Providers

1. Implement the `MmdbProvider` trait
2. Define the MMDB format and field mappings
3. Register the provider in `ProviderRegistry::default()`
4. Update documentation in `docs/src/guide.md`

## Testing Strategies

- Mock MMDB files in tests to avoid external dependencies
- Test both decorating and tagging modes
- Test various input formats (IPv4, IPv6, invalid)
- Test environment variable handling

## Performance Considerations

- **IP Extraction**: Uses compile-time DFA generation via regex-automata for O(n) scanning
- **Zero-copy semantics**: Returns byte ranges instead of allocating strings
- **Caching**: FxHashMap with explicit check-then-insert (not entry API) to avoid double-lookup
- **Lookups**: Cache MMDB results to avoid redundant database access
- **Streaming**: Use buffered I/O and line buffers for large files
- **Fast path**: Use `-j/--justips` when geolocation not needed (65-72x faster than with MMDB)

## Dependencies

- **ip-extract**: Zero-copy IP extraction with compile-time DFA (internal crate)
- **maxminddb**: MMDB reading with mmap support
- **clap**: Command-line argument parsing with env/derive features
- **anyhow**: Error handling with context
- **serde/serde_json**: JSON serialization for tag mode
- **termcolor**: Colored terminal output
- **camino**: UTF-8 path handling
- **thiserror**: Error type derivation
- **bstr**: Byte string utilities

## Helpful Resources

- MaxMind GeoIP2 format: https://maxmind.github.io/MaxMind-DB/
- IP2Location: https://www.ip2location.com/
- IPinfo: https://ipinfo.io/
- https://github.com/sapics/ip-location-db (collection of free IP databases)

## Common Commands

- `just test`: Run all tests
- `just bench`: Run benchmarks
- `just docs-build`: Build documentation locally (mdBook + cargo doc)
- `just docs-serve`: Serve documentation locally on port 3000

## Commit Conventions

- Use imperative mood in commit messages (e.g., "Add IPinfo support" not "Added IPinfo support")
- Include the component being modified (e.g., "extractor: Improve IPv6 validation")
- Reference issues where applicable (e.g., "Fix #23: Handle broken pipes gracefully")
