set shell := ["zsh", "-c"]

_default:
    @just --list

# Run tests
test:
    cargo test --all-targets

# Run benchmarks
bench:
    cargo bench

# Build API documentation from Cargo docs
_build-api-docs:
    cargo doc --workspace --no-deps
    rm -rf docs/src/api
    mkdir -p docs/src/api
    cp -r target/doc/* docs/src/api/
    echo '<meta http-equiv="refresh" content="0; url=geoipsed/index.html">' > docs/src/api/index.html

# Build documentation locally
docs-build: _build-api-docs
    mdbook build docs

# Serve documentation locally with live reload
docs-serve: _build-api-docs
    @echo "Starting mdbook serve on http://localhost:3000"
    mdbook serve docs -p 3000
