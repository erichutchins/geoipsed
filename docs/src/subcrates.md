# Subcrates

The `geoipsed` project is organized as a workspace with several specialized subcrates.

## ip-extract

The core engine for finding and validating IP addresses in strings. It uses a compile-time DFA for O(n) scanning performance.

- [User Guide](ip-extract.md)
- [API Reference](api/ip_extract/index.html)

## ipextract (Python)

A Python package wrapping `ip-extract` via PyO3/maturin. Install with `pip install ipextract`.

- [User Guide](python.md)
- [PyPI](https://pypi.org/project/ipextract/)
