# Subcrates

The `geoipsed` project is organized as a workspace with several specialized subcrates.

## ip-extract

The core engine for finding and validating IP addresses in strings. It uses a compile-time DFA for O(n) scanning performance.

### Defang Support

Defanged IP addresses (`192[.]168[.]1[.]1`, `2001[:]db8[:]0[:]0[:]0[:]0[:]0[:]1`) are recognized
automatically — no configuration needed. The DFA pattern subsumes normal notation,
so there is no performance cost on normal input.

Callers that need the normalized form:
- Use `IpMatch::as_str()` — returns `Cow<str>`, zero-copy for fanged input, strips brackets for defanged
- `IpMatch::as_matched_str()` — returns the raw matched bytes (may contain brackets)
- `IpMatch::ip()` always returns a parsed `IpAddr` (brackets stripped internally)

- [User Guide](ip-extract.md)
- [API Reference](api/ip_extract/index.html)

## justips

A standalone CLI for fast IP extraction. Uses parallel mmap + rayon for maximum throughput on files, with built-in deduplication.

- `-u` / `--unique` — unordered dedup (HashSet, fastest)
- `-U` / `--unique-ordered` — first-seen order (IndexSet)

Benchmarked at **857ms** on 1.7GB Suricata logs (7x faster than ripgrep).

- [User Guide](justips.md)

## ipextract (Python)

A Python package wrapping `ip-extract` via PyO3/maturin. Install with `pip install ipextract`.

- [User Guide](python.md)
- [PyPI](https://pypi.org/project/ipextract/)
