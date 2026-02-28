# Design: `ipextract` Python Bindings via Maturin

**Date**: 2026-02-27
**Status**: Approved

## Overview

Publish a PyPI package `ipextract` that wraps the `ip-extract` Rust crate via PyO3/maturin. Primary audience: data/log analysis workflows in Python (pandas, polars, SOC scripting). The binding exposes a fluent `Extractor` class plus module-level convenience functions.

## Location

New crate at `crates/ipextract-py/`, alongside the existing `crates/ip-extract/`. Not a feature flag on the existing crate — kept separate to avoid polluting the core library's dependency tree with PyO3.

## Python API

```python
import ipextract

# Module-level convenience (default: all IPs included)
ips: list[str] = ipextract.extract("text with 1.2.3.4 and ::1")
ips: list[str] = ipextract.extract_unique("1.2.3.4 1.2.3.4 2.2.2.2")

# Reusable Extractor — constructor kwargs for one-shot config
e = ipextract.Extractor(private=False, loopback=False)

# Fluent builder — each method returns a new Extractor (immutable/thread-safe)
e = ipextract.Extractor().only_public()
e = ipextract.Extractor().ipv4(False).ignore_loopback()

# Extraction methods — accept str or bytes
e.extract(text)               # list[str]
e.extract_unique(text)        # list[str], first-seen order
e.extract_with_offsets(text)  # list[tuple[str, int, int]] — (ip, start, end)
```

## File Structure

```
crates/ipextract-py/
├── Cargo.toml           # [lib] crate-type = ["cdylib"], pyo3 abi3-py310
├── pyproject.toml       # maturin build backend, package metadata
├── src/
│   └── lib.rs           # #[pymodule], PyExtractor #[pyclass]
├── python/
│   └── ipextract/
│       ├── __init__.pyi # type stubs
│       └── py.typed     # PEP 561 marker
└── tests/
    └── test_ipextract.py
```

## Rust Implementation Design

### `PyExtractor` struct

Holds config booleans directly. On construction (or fluent method call), immediately builds the inner `ip_extract::Extractor` and stores it. Fluent methods return a new `PyExtractor` with modified config.

```rust
#[pyclass(name = "Extractor")]
struct PyExtractor {
    inner: ip_extract::Extractor,
    // Config stored for repr/copy in fluent methods
    include_private: bool,
    include_loopback: bool,
    include_broadcast: bool,
    include_ipv4: bool,
    include_ipv6: bool,
}
```

Rebuilding the `Extractor` is essentially free — it just selects one of three pre-built static DFAs. No need for lazy init.

### Fluent builder methods

Each returns a new `Py<PyExtractor>` built from cloned config:

```rust
fn only_public(&self, py: Python<'_>) -> PyResult<Py<PyExtractor>> {
    PyExtractor::from_config(py, self.include_ipv4, self.include_ipv6,
        false, false, false)
}
```

### Input handling

Accept `&[u8]` via PyO3's `#[pyo3(from_py_with = ...)]` or by accepting `PyObject` and downcasting to `PyBytes` or `PyString`. Prefer `PyBytes` path for zero-copy; encode `str` to UTF-8 bytes on the Rust side.

### Exposed functions

```rust
#[pyfunction]
fn extract(text: &[u8]) -> PyResult<Vec<String>> { ... }

#[pyfunction]
fn extract_unique(text: &[u8]) -> PyResult<Vec<String>> { ... }
```

## Packaging

### `Cargo.toml` key settings

```toml
[lib]
name = "ipextract"
crate-type = ["cdylib"]

[dependencies]
ip-extract = { path = "../ip-extract" }
pyo3 = { version = "0.22", features = ["abi3-py310"] }
```

### `pyproject.toml` key settings

```toml
[build-system]
requires = ["maturin>=1.7,<2"]
build-backend = "maturin"

[project]
name = "ipextract"
requires-python = ">=3.10"
classifiers = ["Programming Language :: Rust", "Topic :: Text Processing"]

[tool.maturin]
python-source = "python"
module-name = "ipextract._ipextract"
features = ["pyo3/extension-module"]
```

The native module is `ipextract._ipextract`; the `python/ipextract/__init__.py` re-exports everything for a clean `import ipextract` experience.

## Type Stubs

Ship `.pyi` stubs in `python/ipextract/__init__.pyi` and a `py.typed` marker for PEP 561 compliance. IDEs get full completion and type checking.

```python
class Extractor:
    def __init__(self, *, private: bool = True, loopback: bool = True,
                 broadcast: bool = True, ipv4: bool = True, ipv6: bool = True) -> None: ...
    def only_public(self) -> "Extractor": ...
    def ipv4(self, include: bool) -> "Extractor": ...
    def ipv6(self, include: bool) -> "Extractor": ...
    def ignore_private(self) -> "Extractor": ...
    def ignore_loopback(self) -> "Extractor": ...
    def ignore_broadcast(self) -> "Extractor": ...
    def private_ips(self, include: bool) -> "Extractor": ...
    def loopback_ips(self, include: bool) -> "Extractor": ...
    def broadcast_ips(self, include: bool) -> "Extractor": ...
    def extract(self, text: str | bytes) -> list[str]: ...
    def extract_unique(self, text: str | bytes) -> list[str]: ...
    def extract_with_offsets(self, text: str | bytes) -> list[tuple[str, int, int]]: ...

def extract(text: str | bytes) -> list[str]: ...
def extract_unique(text: str | bytes) -> list[str]: ...
```

## CI / GitHub Actions

Matrix using `PyO3/maturin-action`:

| Platform | Targets |
|---|---|
| Linux | `x86_64`, `aarch64` (cross-compile via QEMU) |
| macOS | `x86_64-apple-darwin`, `aarch64-apple-darwin` |
| Windows | `x86_64-pc-windows-msvc` |

Single `abi3-py310` wheel per platform (not per Python version). Publish to PyPI on `v*` tag push using trusted publishing (OIDC).

## Testing

`tests/test_ipextract.py` covers:
- `extract()` and `extract_unique()` convenience functions
- Default `Extractor()` behavior
- Fluent builder: `.only_public()`, `.ipv4(False)`, chaining
- Constructor kwargs
- `extract_with_offsets()` offset correctness
- `bytes` input path
- Invalid config (no IP version selected → raises `ValueError`)

## Workspace Integration

Add `crates/ipextract-py` to the workspace `members` in the root `Cargo.toml` so it benefits from shared dependency resolution and `cargo check` covers it.
