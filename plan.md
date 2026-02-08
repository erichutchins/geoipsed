# geoipsed Refactor Plan

Goal: Bring the codebase to the quality bar set by BurntSushi's ripgrep — correct,
zero-cost where possible, honest APIs, composable as a library, and measurably fast.

Every phase includes a benchmark checkpoint so we can see whether we're moving the
needle or just shuffling code.

---

## Phase 0: Establish Benchmarks (do first, touch nothing else)

Before changing a single line of production code, set up repeatable micro and macro
benchmarks so every subsequent phase has a before/after story.

### 0a. Add `criterion` benchmarks

Add a `benches/` directory with criterion benchmarks covering the hot paths:

| Benchmark | What it measures |
|-----------|-----------------|
| `extract_ipv4` | `Extractor::find_iter` on a buffer with 1000 IPv4 lines |
| `extract_ipv6` | Same, IPv6 |
| `extract_mixed` | Mixed IPv4+IPv6, simulating real logs |
| `lookup_cached` | `GeoIPSed::lookup` hitting the cache 100% of the time |
| `lookup_cold` | `GeoIPSed::lookup` with all-unique IPs (0% cache hit) |
| `tagged_write` | `Tagged::write` with 1, 2, 5, 20 tags per line |
| `tagged_json` | `Tagged::write_json` same spread |
| `template_render` | Template substitution in isolation |
| `end_to_end` | Full pipeline: read line → extract → lookup → write |

Add to `Cargo.toml`:

```toml
[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }

[[bench]]
name = "benchmarks"
harness = false
```

Keep the existing `benchmark.sh` for wall-clock comparisons of the compiled binary
(hyperfine). The criterion benchmarks are for micro-level regression tracking.

### 0b. Baseline numbers

Run `cargo bench` and `benchmark.sh` on the current code. Record the numbers in a
`BENCHMARKS.md` file so future phases can reference them. Nothing needs to be committed
to CI yet — local baseline is fine for now.

---

## Phase 1: Kill `Box::leak()` — Fix the Memory Leak

**Files:** `src/geoip.rs`

The `lookup()` method leaks memory via `Box::leak()` to manufacture `'static` lifetimes
for `asnnum`, `latitude`, and `longitude` fields. Every unique value is leaked forever.

### Changes

1. Change `IPInfo` to own its numeric fields:

```rust
struct IPInfo {
    ip: String,
    asnnum: String,
    asnorg: String,
    city: String,
    continent: String,
    country_iso: String,
    country_full: String,
    latitude: String,
    longitude: String,
    timezone: String,
}
```

2. Drop the `microtemplate` dependency for `IPInfo` rendering. `microtemplate` requires
   `&'a str` fields via the `Substitutions` derive, which is why the leak was introduced.
   Replace with a simple single-pass template renderer (see Phase 5) or use
   `microtemplate` with owned `String` fields if it supports them via `Cow`.

3. If `microtemplate` can't work with owned data, write a minimal `render()` function
   that takes `&IPInfo` and a template `&str`, doing a single left-to-right scan. This
   also fixes the double-substitution bug in `mmdb.rs::apply_template`.

4. Remove `field_names` derive from `IPInfo` — it was only needed for `microtemplate`.

### Benchmark checkpoint

Run `lookup_cold` and `lookup_cached` before/after. The owned-String approach may add a
small allocation cost per lookup miss, but eliminates unbounded growth. Net memory usage
on a 1M-line log should drop dramatically.

---

## Phase 2: Zero-Allocation Hot Path

**Files:** `src/main.rs`, `src/geoip.rs`

The inner match loop currently clones strings 1-3 times per IP match depending on mode.
The goal is: **zero allocations on cache hit, one allocation on cache miss**.

### 2a. Non-tag mode (decoration)

Current problem in `run()`:

```rust
// Called unconditionally — allocates even on cache hit
let decorated = cache
    .entry(ipstr.to_string())
    .or_insert_with(|| geoipdb.lookup(ipstr));
```

Fix: check-then-insert pattern:

```rust
if let Some(cached) = cache.get(ipstr) {
    out.write_all(cached.as_bytes())?;
} else {
    let result = geoipdb.lookup(ipstr);
    out.write_all(result.as_bytes())?;
    cache.insert(ipstr.to_owned(), result);
}
```

### 2b. Tag mode

Current problem: 3 clones per IP (`ip_buffer.clone()`, `entry(ipstr.clone())`,
`.clone()` on the result).

Fix: use the same check-then-insert pattern. For the `Tag`, pass a `&str` borrow where
possible, only converting to owned `String` when building the `Tag` struct (which needs
ownership for serialization).

### 2c. Remove the `thread_local!` cache in `geoip.rs`

The `IP_CACHE` thread-local in `geoip.rs` duplicates the cache in `main.rs`. There are
now two caches: one in `run()` and one inside `GeoIPSed::lookup()`. Remove the
thread-local. The caller owns the cache.

This means `GeoIPSed::lookup()` becomes a pure function: IP in, decorated string out, no
hidden state.

### 2d. Fix the fast-path bug

```rust
if !tag_mode && extractor.find_iter(haystack).count() == 0 {
```

This runs the extractor **twice** on lines with matches (once to count, once to
iterate). Fix: just iterate once and check if the iterator yields anything, or always
enter the match loop and let the "no matches" case fall through naturally (write the
original line if no tags were added).

### Benchmark checkpoint

Run `end_to_end` and `extract_mixed` before/after. Expect meaningful improvement on
cache-hot workloads (repeated IPs in logs). Memory profiling with `dhat` or `heaptrack`
should show flat memory usage instead of growth.

---

## Phase 3: Delete Stub Providers

**Files:** `src/mmdb.rs`

`IP2LocationProvider` and `IPinfoProvider` have `lookup()` methods that return placeholder
strings. They don't work. Shipping fake implementations makes the provider system look
broken and makes `--list-providers` misleading.

### Changes

1. Remove `IP2LocationProvider` and `IPinfoProvider` entirely.
2. Remove them from `ProviderRegistry::default()`.
3. Keep the `MmdbProvider` trait and `ProviderRegistry` — they're well-designed and the
   right extension point. Just don't ship implementations that lie.
4. Add a comment in `mmdb.rs` and a note in `GUIDE.md` explaining how to add a provider
   when the MMDB schema is known.
5. Remove the `define_mmdb_provider!` and `register_mmdb_provider!` macros. They generate
   providers with stub `lookup()` methods and are untested. When a real second provider
   is needed, write a proper implementation.

### 3b. Fix `apply_template` double-substitution bug

The current naive loop replaces `{field}` placeholders iteratively, so a value containing
`{other_field}` gets double-replaced. Replace with a single-pass scan. This may be
unified with the renderer from Phase 1.

### 3c. Remove string-based test detection

```rust
let is_test_dir = path.to_string_lossy().contains("tests/maxmind")
    || path.to_string_lossy().contains("test");
```

Replace with `#[cfg(test)]` conditional logic or pass a configuration flag
(`allow_missing_files: bool`).

### Benchmark checkpoint

Compile time should improve slightly (less code). No runtime regression expected.

---

## Phase 4: Separate Concerns — Processor / Printer Split

**Files:** `src/main.rs` (refactor into `src/processor.rs` and update `main.rs`)

The `run()` function is ~200 lines mixing I/O, matching, caching, and output formatting.
This makes it impossible to test the processing logic without actual MMDB files and real
I/O.

### Changes

1. Extract a `Processor` struct:

```rust
pub struct Processor<'a> {
    extractor: &'a Extractor,
    geoip: &'a GeoIPSed,
    cache: FxHashMap<String, String>,
}

impl<'a> Processor<'a> {
    /// Process a single line. Returns an iterator or vec of (range, decorated) pairs.
    pub fn process_line(&mut self, line: &[u8]) -> Vec<(Range<usize>, String)> { ... }

    /// Look up a single IP, using the cache.
    pub fn lookup(&mut self, ip: &str) -> &str { ... }
}
```

2. Keep output logic in `main.rs` or a thin `printer.rs`. The printer takes decorated
   results and writes them in the appropriate mode (plain text, colored, JSON).

3. `run()` becomes:

```rust
fn run(args: Args, colormode: ColorChoice) -> Result<()> {
    let geoip = GeoIPSed::new(...)?;
    let extractor = ExtractorBuilder::new()...build()?;
    let mut processor = Processor::new(&extractor, &geoip);
    let mut out = StandardStream::stdout(colormode);

    for path in args.input {
        let mut reader = FileOrStdin::from_path(path).reader()?;
        reader.for_byte_line(|line| {
            let matches = processor.process_line(line.content());
            write_output(&mut out, line, &matches, tag_mode)?;
            Ok(true)
        })?;
    }
    Ok(())
}
```

### Benefit

- `Processor` is testable with fake lookup functions and in-memory data
- Output modes can be tested independently
- The library surface (`lib.rs`) exposes composable pieces instead of a monolithic `run()`

### Benchmark checkpoint

No performance change expected — this is a structural refactor. Verify with `end_to_end`
that we haven't regressed.

---

## Phase 5: Single-Pass Template Renderer

**Files:** new `src/template.rs`, update `src/geoip.rs` and `src/mmdb.rs`

Both `geoip.rs` (via `microtemplate`) and `mmdb.rs` (via `apply_template`) render
templates. Unify them into a single, correct, fast implementation.

### Design

```rust
/// A pre-compiled template for fast rendering.
pub struct Template {
    /// Segments of literal text and field references, in order.
    parts: Vec<TemplatePart>,
}

enum TemplatePart {
    Literal(String),
    Field(String),  // field name without braces
}

impl Template {
    /// Parse a template string like "<{ip}|{country_iso}>".
    pub fn compile(template: &str) -> Result<Template> { ... }

    /// Render the template with the given field values.
    /// Single pass, no double-substitution possible.
    pub fn render(&self, values: &impl Fn(&str) -> &str) -> String { ... }
}
```

### Changes

1. Create `src/template.rs` with the above.
2. `GeoIPSed::lookup()` uses `Template::render()` instead of `microtemplate::render()`.
3. `MaxMindProvider::lookup()` uses `Template::render()` instead of `apply_template()`.
4. Remove `microtemplate` and `field_names` from `Cargo.toml`.
5. Remove the `apply_template` function from `mmdb.rs`.
6. Compile the template once at startup (in `GeoIPSed::new()`) instead of re-parsing
   it on every `lookup()` call.

### Benchmark checkpoint

Run `template_render` before/after. Expect improvement from pre-compilation: the current
code parses the template string on every single lookup call.

---

## Phase 6: Clean Up API Honesty

**Files:** `src/geoip.rs`, `src/lib.rs`, `src/extractor.rs`

### 6a. Remove panicking `Default` impl

`GeoIPSed::default()` calls `.expect()`, which panics if MMDB files aren't found.
Delete the `Default` impl. Construction should only go through `GeoIPSed::new()` which
returns `Result`.

### 6b. Remove `#[inline(always)]` overuse

`#[inline(always)]` on `validate_ipv4`, `validate_ipv6`, `Tag::new`, `Tag::ip`, every
builder setter, `Tagged::write`, `is_broken_pipe`, etc. Most of these are in the same
crate, so LLVM already inlines them when profitable. `#[inline(always)]` can hurt
instruction cache pressure.

**Keep** `#[inline]` (without `always`) on public API methods that cross crate boundaries
(e.g., `Extractor::find_iter`, `Tagged::write`). Remove `always` everywhere else.

### 6c. Proper error types for the library

Create `src/error.rs`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database not found: {path}")]
    DatabaseNotFound { path: PathBuf },

    #[error("lookup failed for {ip}: {source}")]
    LookupFailed {
        ip: String,
        #[source]
        source: maxminddb::MaxMindDBError,
    },

    #[error("invalid template: {reason}")]
    InvalidTemplate { reason: String },

    #[error("no IP patterns selected")]
    NoPatterns,

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
```

The binary (`main.rs`) can still use `anyhow` to wrap these. The library surface returns
`Result<T, geoipsed::Error>`.

Add `thiserror` to `[dependencies]`.

### 6d. Document the IPv6 regex

The 500-character IPv6 pattern in `extractor.rs` has no explanation. Either:
- Build it programmatically from documented components, or
- Add a block comment explaining each alternation branch

### Benchmark checkpoint

No performance change expected. Verify no regression.

---

## Phase 7: Minor Wins

### 7a. `files.rs` — eliminate duplication

`tag_file()` and the stdin branch in `tag_files()` are nearly identical. Extract a
`tag_content(content: &[u8], extractor, output)` helper.

### 7b. `files.rs` — avoid `from_utf8_lossy().to_string()`

```rust
let ip_str = String::from_utf8_lossy(ip_slice).to_string();
```

This double-allocates. Since IP addresses are always ASCII, use `str::from_utf8()` and
return an error on non-UTF8 (which can't happen for valid IPs).

### 7c. `tag.rs` — avoid clone in unsorted path

```rust
let mut sorted_tags = self.tags.clone();
```

Use a `Vec<usize>` of indices and sort that instead. Or, since the extractor always finds
matches left-to-right, assert sortedness with a `debug_assert!` and remove the fallback
entirely.

### 7d. `tag.rs` — avoid allocation in `write_json`

```rust
self.text_data = Some(TextData { text: s.to_string() });
```

This copies the entire line text into an owned String just for serialization. Consider
using `serde_json::to_writer` with a custom serializer that borrows from `&self.text`,
or use `Cow<'a, str>`.

### 7e. `input.rs` — deduplicate `read_until` branches

```rust
let n = match self {
    InputReader::File(ref mut rdr) => rdr.read_until(b'\n', &mut buf),
    InputReader::Stdin(ref mut rdr) => rdr.read_until(b'\n', &mut buf),
};
```

Consider implementing `BufRead` for `InputReader` via delegation, or use a trait object
`Box<dyn BufRead>` (acceptable overhead since the real cost is the syscall, not the
vtable dispatch).

### 7f. CI — run tests

The GitHub Actions workflow runs clippy and rustfmt but not `cargo test`. Add it.

### Benchmark checkpoint

Run full suite. 7b and 7d may show small improvements on tag/JSON workloads.

---

## Phase Summary

| Phase | Type | Risk | Impact |
|-------|------|------|--------|
| 0 | Benchmarks | None | Foundation for all other work |
| 1 | Correctness | Low | Fixes unbounded memory leak |
| 2 | Performance | Medium | Zero-alloc hot path, ~2x cache-hit throughput |
| 3 | Cleanup | Low | Removes dead code, fixes substitution bug |
| 4 | Architecture | Medium | Testable processor, composable library |
| 5 | Performance | Low | Pre-compiled templates, single-pass render |
| 6 | API quality | Low | Honest errors, no panics, documented patterns |
| 7 | Polish | Low | Small wins across the board |

Phases 0-3 are the highest priority — they fix real bugs and establish measurement.
Phases 4-5 are architectural improvements that enable better testing and library reuse.
Phases 6-7 are polish.

---

## Dependencies Changes Summary

| Add | Remove |
|-----|--------|
| `criterion` (dev) | `microtemplate` |
| `thiserror` | `field_names` |
| | (stub providers code) |

---

## Files Changed/Created Summary

| File | Action |
|------|--------|
| `benches/benchmarks.rs` | Create — criterion benchmarks |
| `src/template.rs` | Create — single-pass template engine |
| `src/processor.rs` | Create — extract from main.rs |
| `src/error.rs` | Create — library error types |
| `src/geoip.rs` | Major — remove leak, remove thread_local, use template.rs |
| `src/main.rs` | Major — extract processor, fix hot-path allocations |
| `src/mmdb.rs` | Major — remove stubs, remove apply_template, remove macros |
| `src/tag.rs` | Minor — index-based sorting, borrow in JSON |
| `src/files.rs` | Minor — dedup, fix double-alloc |
| `src/input.rs` | Minor — dedup read_until |
| `src/extractor.rs` | Minor — document regex, fix inline attrs |
| `src/lib.rs` | Update — re-export error types |
| `Cargo.toml` | Update dependencies |
| `BENCHMARKS.md` | Create — baseline numbers |
