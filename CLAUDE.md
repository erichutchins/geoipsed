# CLAUDE.md - Coding Conventions and Patterns for geoipsed

## General Principles

- **Modularity**: Keep related functionality in separate modules (e.g., extract, geoip, mmdb)
- **Extensibility**: Use traits and builder patterns to allow easy extension
- **Error Handling**: Use `anyhow` consistently with context for all errors
- **Documentation**: Document public APIs and add examples where relevant
- **Performance first**: Benchmark before and after changes (use criterion for throughput comparisons)

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
├── crates/
│   ├── ip-extract/              # IP extraction crate (DFA-based, zero-copy)
│   │   ├── src/lib.rs           # Extractor, builder pattern, defang support
│   │   ├── build.rs             # Compile-time DFA generation
│   │   ├── benches/ip_benchmark.rs  # Criterion benchmarks (extraction, parsing, defang)
│   │   └── tests/extract_tests.rs   # 77+ comprehensive extraction tests
│   └── ipextract-py/            # Python bindings (PyO3 + maturin, pushed to PyPI)
│       ├── src/lib.rs           # PyO3 Extractor, builder, convenience functions
│       ├── pyproject.toml       # Maturin config (abi3-py310 stable ABI)
│       ├── python/ipextract/
│       │   ├── __init__.py      # Re-exports from native module
│       │   ├── __init__.pyi     # PEP 561 type stubs
│       │   └── py.typed         # PEP 561 marker file
│       └── tests/test_ipextract.py  # 25+ Python tests
├── docs/
│   ├── src/
│   │   ├── python.md            # ipextract Python package docs
│   │   ├── subcrates.md         # ip-extract and ipextract-py overview
│   │   └── ...
│   └── plans/                   # Design docs (not committed)
├── .github/workflows/
│   └── publish-ipextract.yml    # Maturin: build wheels (Linux/macOS/Windows) + PyPI publish
├── tests/
│   └── maxmind/                 # MMDB test fixtures
└── python/
    └── geoipsed.py              # Python reference implementation (benchmarking)
```

### Module Dependencies

- **IP Extraction**: `ip-extract` crate (library crate, DFA-based zero-copy, defang-capable)
- **Python Bindings**: `ipextract-py` crate (PyO3 bindings, maturin-built, stable ABI abi3-py310)
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

// Defang support is always-on — no opt-in needed.
// Defanged IPs (192[.]168[.]1[.]1, 2001[:]db8[:]0[:]0[:]0[:]0[:]0[:]1) are matched by the default DFA.
// Note: [::] compression is NOT supported in defanged notation — use fully-expanded form.
let extractor = ExtractorBuilder::new().build()?;
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

### Defanged IP Support

Defanged IPs (`192[.]168[.]1[.]1`, `2001[:]db8[:]0[:]0[:]0[:]0[:]0[:]1`) are recognized automatically —
always-on, no configuration needed. The DFA pattern subsumes normal notation, so there
is no performance cost on normal input (~504 MiB/s on fanged IPv4).

**Key decision** (benchmark spike 2026-02-28): Use DFA expansion over pre-processing
normalization. The expanded DFA is 16% faster on defanged input (434 vs 374 MiB/s),
has negligible binary overhead (+3 KB), and adds no regression on normal input.

Always use refanged (normal) IPs for MMDB lookups, cache keys, and output:
- `IpMatch::as_str()` — returns `Cow<str>`, zero-copy for fanged input, cleans defang brackets
- `IpMatch::as_matched_str()` — raw matched bytes (may contain brackets), useful for logging original input
- `IpMatch::ip()` — parses to `IpAddr`, strips brackets internally

### Python Bindings Pattern (PyO3 + maturin)

Use the fluent builder pattern in Python, matching Rust semantics:

```python
from ipextract import Extractor

# Builder-style configuration
extractor = (
    Extractor()
    .ipv4(True)
    .ipv6(True)
    .ignore_private()
)

# Python API exports are stable (abi3-py310)
ips = extract(text)  # str or bytes → list[str]
unique_ips = extract_unique(text)  # list[str], order-preserving

# Extract with offsets and metadata
for ip, start, end in extractor.extract_with_offsets(text):
    print(f"Found {ip} at {start}:{end}")
```

Python API wheels work across 3.10-3.13+ without recompilation. Type stubs (`__init__.pyi`)
and `py.typed` marker ensure IDE support.

## Command-Line Interface Conventions

- Use `--flag` format for boolean options
- Use opt-out negated flags (e.g., `--no-private`) to disable features (principle of least surprise)
- Use `--all` to include all IP types (private, loopback, broadcast)
- Provide both short (`-o`, `-C`, `-t`, `-L`, `-I`) and long forms for common options
- Support environment variables for paths (`GEOIP_MMDB_DIR`)
- For raw IP extraction, use the standalone `justips` crate (parallel mmap, ~2 GiB/s)
- Defang recognition is always-on — no CLI flag needed

## Adding New MMDB Providers

1. Implement the `MmdbProvider` trait
2. Define the MMDB format and field mappings
3. Register the provider in `ProviderRegistry::default()`
4. Update documentation in `docs/src/guide.md`

## Python Package Publishing (ipextract)

### Local Development

```bash
cd crates/ipextract-py
maturin develop  # Build and install in-place for testing
```

### Publishing Workflow

1. Update version in `Cargo.toml` (both `ipextract-py` and `ip-extract` if coordinated)
2. Create commit with message: `release: ipextract-py v0.x.y`
3. Tag with `ipextract-vX.Y.Z` (GitHub Actions triggers on this pattern)
4. Push commits and tags — CI builds wheels for Linux/macOS/Windows and publishes to PyPI

The `.github/workflows/publish-ipextract.yml` handles matrix builds and PyPI auth via secrets.

## Testing Strategies

- **Extraction tests**: Test boundary conditions, overflow detection, IPv4/IPv6 variants (77+ cases)
- **MMDB tests**: Mock MMDB files to avoid external dependencies
- **Tagging modes**: Test both decorating and tagging output
- **Input formats**: Test IPv4, IPv6, invalid, and mixed cases
- **Environment variables**: Test `GEOIP_MMDB_DIR` handling
- **Python tests**: Test builder chains, convenience functions, edge cases (25+ cases)
- **Benchmarks**: Use criterion to validate throughput before/after optimizations

## Performance Considerations

- **IP Extraction**: Compile-time DFA generation via regex-automata for O(n) scanning (~400 MiB/s on dense logs)
- **Scale**: Processes 1.7GB of Suricata NDJSON logs (15.4M lines) in 4.35s (**99x faster than Python**)
  - Patterns include defang variants (`[.]`, `[:]`) at no regression cost on normal input
  - Boundary detection uses 55-char backward scan (wider for bracket notation)
- **Zero-copy semantics**: Returns byte ranges instead of allocating strings
- **Caching pattern**: `FxHashMap` with explicit check-then-insert (NOT `.entry()` API)
  - Avoids double-lookup on cache hit (typical case)
  - More verbose but measurably faster for hot lookups
- **MMDB Lookups**: Cache results by IP to avoid redundant database access
- **Streaming**: Use buffered I/O and line buffers for large files
- **Fast path**: For IP extraction without geolocation, use the standalone `justips` crate (parallel mmap, ~2 GiB/s)
- **Benchmark-driven**: Criterion benchmarks in `crates/ip-extract/benches/` measure throughput
  - IPv4 extraction (sparse logs): ~395 MiB/s
  - Real-world logs (Suricata v4/v6 mixed): ~390 MiB/s (15.4M lines in 4.35s)
  - IPv6 extraction: ~253 MiB/s
  - Dense IPs (mostly IPs): ~152 MiB/s
  - IPv4-only baseline (fanged): ~504 MiB/s
  - Defanged IPv4: ~450 MiB/s (DFA)
  - No regression on fanged input with always-on defang DFA

## Dependencies

### Core (geoipsed binary)

- **ip-extract**: Zero-copy IP extraction with compile-time DFA, defang support (internal)
- **maxminddb**: MMDB reading with mmap support
- **clap**: Command-line argument parsing with env/derive features
- **anyhow**: Error handling with context
- **serde/serde_json**: JSON serialization for tag mode
- **termcolor**: Colored terminal output
- **camino**: UTF-8 path handling
- **thiserror**: Error type derivation
- **bstr**: Byte string utilities

### ip-extract crate

- **regex-automata**: DFA building and searching (build-time + runtime)
- **regex-syntax**: Regex pattern parsing (build-time)
- **memchr**: Fast byte scanning for defang normalization
- **anyhow, serde, serde_json**: Standard tooling

### ipextract-py crate

- **pyo3**: PyO3 bindings (abi3-py310 feature for stable ABI)
- **maturin**: Build backend for Python wheels
- **ip-extract**: (internal, re-exported as `ipextract` on PyPI)

## Helpful Resources

- MaxMind GeoIP2 format: https://maxmind.github.io/MaxMind-DB/
- IP2Location: https://www.ip2location.com/
- IPinfo: https://ipinfo.io/
- https://github.com/sapics/ip-location-db (collection of free IP databases)
- PyO3 guide: https://pyo3.rs/ (Python bindings for Rust)
- Maturin: https://www.maturin.rs/ (build and publish Rust/Python packages)

## Common Commands

- `just test`: Run all Rust tests (workspace-wide: geoipsed + ip-extract + ipextract-py native)
- `just bench`: Run criterion benchmarks (extraction, parsing, defang)
- `just docs-build`: Build documentation locally (mdBook + cargo doc)
- `just docs-serve`: Serve documentation locally on port 3000
- `cd crates/ipextract-py && maturin develop`: Build Python bindings for local testing
- `cd crates/ipextract-py && maturin build --release`: Build distribution wheels

## Commit Conventions

- Use imperative mood in commit messages (e.g., "Add IPinfo support" not "Added IPinfo support")
- Include the component being modified (e.g., "extractor: Improve IPv6 validation")
- Reference issues where applicable (e.g., "Fix #23: Handle broken pipes gracefully")
- For Python releases: use `release: ipextract-py vX.Y.Z` and tag as `ipextract-vX.Y.Z`

## Key Learnings & Gotchas

- **Memory leaks via Box::leak()**: Never manufacture `'static` lifetimes for pool-allocated data.
  Use owned `String` instead. See geoipsed v0.1 refactoring for details.
- **Double-lookup in caches**: `.entry().or_insert_with()` allocates on hit. Use explicit
  `if let Some(value) = cache.get()` followed by `cache.insert()` in else branch.
- **DFA boundary detection**: The 55-char backward scan accommodates bracket notation
  (max defanged IPv6 ~53 chars). Uses `is_ip_or_bracket_char` for bracket-aware boundaries.
- **Defang performance**: DFA expansion beats pre-processing normalization by 16%.
  - Supports only single-colon brackets (e.g. `[:]`) and dots (`[.]`).
  - `[::]` compression in defanged notation is NOT supported (fully-expanded notation required for IPv6).
  - On normal (fanged) input, the per-match overhead is a ~15-byte memchr scan that
  - finds nothing — negligible cost confirmed by benchmarks.
