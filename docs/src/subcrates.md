# Subcrates

The `geoipsed` project is organized as a workspace with several specialized subcrates.

## ip-extract

The core engine for finding and validating IP addresses in strings. It uses a compile-time DFA for O(n) scanning performance.

### Defang Support

Defanged IP addresses (`192[.]168[.]1[.]1`, `2001[:]db8[::]1`) are recognized
automatically — no configuration needed. The DFA pattern subsumes normal notation,
so there is no performance cost on normal input (~493 MiB/s regardless).

Callers that need the normalized form:
- Use `IpMatch::as_str_refanged()` — returns `Cow<str>`, zero-copy for fanged input
- `IpMatch::ip()` always returns a parsed `IpAddr` (brackets stripped internally)

- [User Guide](ip-extract.md)
- [API Reference](api/ip_extract/index.html)

## ipextract (Python)

A Python package wrapping `ip-extract` via PyO3/maturin. Install with `pip install ipextract`.

- [User Guide](python.md)
- [PyPI](https://pypi.org/project/ipextract/)
