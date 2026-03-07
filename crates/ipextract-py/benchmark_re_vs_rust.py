"""
Benchmark: Python re + ipaddress validation vs ipextract (Rust DFA)

Compares two approaches to IP extraction from Python:
  1. Python Baseline: Comprehensive regex + validation (mimics geoipsed.py)
  2. ipextract: Rust DFA-powered extraction engine
"""

import gc
import ipaddress
import re
import time
import random
import sys
import os
from typing import List

# Attempt to load the native module
try:
    import ipextract
except ImportError:
    # If not installed globally, try to find it in the local build artifacts
    # (Useful when running via "maturin develop" and not in a venv)
    sys.path.insert(0, os.path.join(os.getcwd(), "python"))
    try:
        import ipextract
    except ImportError:
        print("Could not import ipextract. Please build it first:")
        print("  cd crates/ipextract-py && maturin develop")
        sys.exit(1)

# ---------------------------------------------------------------------------
# Python Baseline Implementation (Improved from geoipsed.py)
# ---------------------------------------------------------------------------

# Precise IPv4 pattern
IPV4_RE = r"(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)"

# Precise IPv6 pattern (supporting various RFC formats)
IPV6_RE = r"(?:(?:(?:[0-9a-fA-F]{1,4}:){6}(?:(?:[0-9a-fA-F]{1,4}:[0-9a-fA-F]{1,4})|(?:\d{1,3}\.){3}\d{1,3}))|(?:::(?:[0-9a-fA-F]{1,4}:){0,5}(?:(?:[0-9a-fA-F]{1,4}(?::[0-9a-fA-F]{1,4})?)|(?:\d{1,3}\.){3}\d{1,3}))|(?:[0-9a-fA-F]{1,4}::(?:[0-9a-fA-F]{1,4}:){0,4}(?:(?:[0-9a-fA-F]{1,4}(?::[0-9a-fA-F]{1,4})?)|(?:\d{1,3}\.){3}\d{1,3}))|(?:(?:[0-9a-fA-F]{1,4}:){2}::(?:[0-9a-fA-F]{1,4}:){0,3}(?:(?:[0-9a-fA-F]{1,4}(?::[0-9a-fA-F]{1,4})?)|(?:\d{1,3}\.){3}\d{1,3}))|(?:(?:[0-9a-fA-F]{1,4}:){3}::(?:[0-9a-fA-F]{1,4}:){0,2}(?:(?:[0-9a-fA-F]{1,4}(?::[0-9a-fA-F]{1,4})?)|(?:\d{1,3}\.){3}\d{1,3}))|(?:(?:[0-9a-fA-F]{1,4}:){4}::(?:[0-9a-fA-F]{1,4}:){0,1}(?:(?:[0-9a-fA-F]{1,4}(?::[0-9a-fA-F]{1,4})?)|(?:\d{1,3}\.){3}\d{1,3}))|(?:(?:[0-9a-fA-F]{1,4}:){5}::(?:(?:[0-9a-fA-F]{1,4}(?::[0-9a-fA-F]{1,4})?)|(?:\d{1,3}\.){3}\d{1,3}))|(?:(?:[0-9a-fA-F]{1,4}:){6}::[0-9a-fA-F]{1,4}))"

# Combined pattern with negative lookarounds/lookaheads to prevent matching
# partial IPs embedded in other numbers or hex strings.
IP_RE = re.compile(
    rf"(?<![a-f0-9.:])(?:({IPV4_RE})|({IPV6_RE}))(?![a-f0-9.:])", re.IGNORECASE
)


def python_extract(text: str) -> List[str]:
    """Pure Python extraction using Regex + ipaddress validation."""
    results = []
    for match in IP_RE.finditer(text):
        ip_str = match.group(0)
        try:
            # Validate via stdlib to match Rust's internal validation
            ipaddress.ip_address(ip_str)
            results.append(ip_str)
        except ValueError:
            continue
    return results


# ---------------------------------------------------------------------------
# Data Generation
# ---------------------------------------------------------------------------


def generate_log_data(n_ips: int, bytes_per_ip: int = 200) -> str:
    """Generate realistic log-like data."""
    random.seed(42)
    parts = []

    # Typical noise in logs
    noise_templates = [
        "GET /index.html HTTP/1.1 200",
        "POST /api/v1/update HTTP/1.1 403",
        "User-Agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64)",
        "Referer: https://google.com/search?q=rust+performance",
        'Timestamp: 2026-03-06T12:34:56Z level=INFO msg="Connection received"',
    ]

    for i in range(n_ips):
        # Add some noise
        parts.append(random.choice(noise_templates))
        parts.append(" src_ip=")

        # Add an IP
        if i % 2 == 0:
            parts.append(".".join(str(random.randint(0, 255)) for _ in range(4)))
        else:
            # Random IPv6 (sometimes compressed)
            if random.random() > 0.5:
                parts.append(f"2001:db8::{random.randint(0, 0xFFFF):x}")
            else:
                parts.append(
                    ":".join(f"{random.randint(0, 0xFFFF):x}" for _ in range(8))
                )

        parts.append(" proto=tcp port=443\n")

    return "".join(parts)


def generate_dense_data(n_ips: int) -> str:
    """Generate dense IP data."""
    random.seed(42)
    parts = []
    for i in range(n_ips):
        if i % 2 == 0:
            parts.append(".".join(str(random.randint(0, 255)) for _ in range(4)))
        else:
            parts.append(f"2001:db8::{random.randint(0, 0xFFFF):x}")
        parts.append(" " if i % 10 != 0 else "\n")
    return "".join(parts)


def generate_no_ips(size_kb: int) -> str:
    """Generate noise with zero IPs."""
    random.seed(42)
    noise_chars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-:[]/ "
    return "".join(random.choices(noise_chars, k=size_kb * 1024))


# ---------------------------------------------------------------------------
# Benchmark Logic
# ---------------------------------------------------------------------------


def bench(name, text, extractor, n_iters_py=5, n_iters_rust=50):
    data_size_kb = len(text.encode("utf-8")) / 1024

    # Warmup
    python_extract(text)
    extractor.extract(text)

    # Benchmark Python
    gc.collect()
    start = time.perf_counter()
    for _ in range(n_iters_py):
        py_matches = python_extract(text)
    py_time = (time.perf_counter() - start) / n_iters_py

    # Benchmark Rust
    gc.collect()
    start = time.perf_counter()
    for _ in range(n_iters_rust):
        rust_matches = extractor.extract(text)
    rust_time = (time.perf_counter() - start) / n_iters_rust

    speedup = py_time / rust_time if rust_time > 0 else 0
    
    print(
        f"{name:<20} | {py_time * 1000:>10.2f} | {rust_time * 1000:>10.2f} | {speedup:>8.1f}x | {len(rust_matches):>8}"
    )


def run_bench():
    print(f"{'Scenario':<20} | {'re (ms)':>10} | {'Rust (ms)':>10} | {'Speedup':>9} | {'Matches':>8}")
    print("-" * 68)

    extractor = ipextract.Extractor()

    # Log Data (Sparse)
    log_text = generate_log_data(1000)
    bench("Sparse Logs", log_text, extractor)

    # Dense IPs
    dense_text = generate_dense_data(1000)
    bench("Dense IPs", dense_text, extractor)

    # No IPs (100KB)
    no_ip_text = generate_no_ips(100)
    bench("Pure Text (100KB)", no_ip_text, extractor, n_iters_py=10, n_iters_rust=100)


if __name__ == "__main__":
    run_bench()
