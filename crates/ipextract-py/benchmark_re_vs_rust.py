"""
Benchmark: Python re + ipaddress validation vs ipextract (Rust DFA)

Compares two approaches to IP extraction from text:
  1. Python stdlib: re.findall() + ipaddress.ip_address() validation
  2. ipextract: Rust DFA-based extraction (validates during scan)

The regex approach uses a common "good enough" pattern, then filters
through ipaddress.ip_address() to reject invalid matches (e.g. 999.1.2.3).
This makes the comparison fair — both sides do extraction + validation.
"""

import gc
import ipaddress
import re
import timeit
import random
import sys
import os

# Add the local python directory to sys.path so we can import ipextract
sys.path.insert(0, os.path.join(os.getcwd(), "python"))

try:
    from ipextract import Extractor
except ImportError:
    print("Could not import ipextract. Please ensure it is built:")
    print("  cd crates/ipextract-py && maturin develop")
    sys.exit(1)

random.seed(42)  # Reproducible data generation

# ---------------------------------------------------------------------------
# Data generators
# ---------------------------------------------------------------------------

def random_ipv4():
    return ".".join(str(random.randint(0, 255)) for _ in range(4))

def random_ipv6():
    return ":".join(f"{random.randint(0, 65535):x}" for _ in range(8))

def random_defanged_ipv4():
    return random_ipv4().replace(".", "[.]")

def random_defanged_ipv6():
    return random_ipv6().replace(":", "[:]")

def generate_dense_ips(n):
    parts = []
    for i in range(n):
        if i % 2 == 0:
            parts.append(random_ipv4())
        else:
            parts.append(random_ipv6())
        parts.append("\n" if i % 10 == 0 else " ")
    return "".join(parts)

def generate_log_data(n_ips, bytes_per_ip):
    noise_chars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-:[]/."
    parts = []
    for i in range(n_ips):
        noise_len = random.randint(bytes_per_ip // 2, bytes_per_ip * 2)
        noise = "".join(random.choice(noise_chars) for _ in range(noise_len))
        parts.append(noise)
        parts.append(" ")
        if i % 2 == 0:
            parts.append(random_ipv4())
        else:
            parts.append(random_ipv6())
        parts.append(" ")
    return "".join(parts)

def generate_no_ips(size):
    noise_chars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-:[]/."
    return "".join(random.choice(noise_chars) for _ in range(size))

def generate_defanged_data(n):
    parts = []
    for i in range(n):
        if i % 2 == 0:
            parts.append(random_defanged_ipv4())
        else:
            parts.append(random_defanged_ipv6())
        parts.append(" ")
    return "".join(parts)

# ---------------------------------------------------------------------------
# Regex patterns (common "good enough" approach)
# ---------------------------------------------------------------------------
# Note: These intentionally match invalid IPs like 999.1.2.3 — the
# ipaddress.ip_address() filter handles validation, same as real-world usage.

IPV4_RE = r"\b(?:[0-9]{1,3}\.){3}[0-9]{1,3}\b"
IPV6_RE = r"(?:[a-fA-F0-9]{1,4}:){2,7}[a-fA-F0-9]{1,4}"
IP_COMBINED_RE = re.compile(f"{IPV4_RE}|{IPV6_RE}")

IPV4_DEFANG_RE = r"\b(?:[0-9]{1,3}(?:\[\.\]|\.)){3}[0-9]{1,3}\b"
IPV6_DEFANG_RE = r"(?:[a-fA-F0-9]{1,4}(?:\[:\]|:)){2,7}[a-fA-F0-9]{1,4}"
IP_DEFANG_COMBINED_RE = re.compile(f"{IPV4_DEFANG_RE}|{IPV6_DEFANG_RE}")

# ---------------------------------------------------------------------------
# Extraction functions
# ---------------------------------------------------------------------------

def regex_extract_validated(text):
    """re.findall() + ipaddress validation — fair comparison to Rust DFA."""
    results = []
    for match in IP_COMBINED_RE.findall(text):
        try:
            ipaddress.ip_address(match)
            results.append(match)
        except ValueError:
            pass
    return results

def regex_defang_extract_validated(text):
    """re.findall() on defanged patterns + ipaddress validation after refanging."""
    results = []
    for match in IP_DEFANG_COMBINED_RE.findall(text):
        refanged = match.replace("[.]", ".").replace("[:]", ":")
        try:
            ipaddress.ip_address(refanged)
            results.append(refanged)
        except ValueError:
            pass
    return results

def regex_extract_unique_validated(text):
    """re.findall() + ipaddress validation + order-preserving unique."""
    return list(dict.fromkeys(regex_extract_validated(text)))

# ---------------------------------------------------------------------------
# Benchmark runner
# ---------------------------------------------------------------------------

N_REPEATS = 5   # timeit.repeat rounds (take the min)
N_ITERS = 100   # iterations per repeat

def bench(fn, repeats=N_REPEATS, iters=N_ITERS):
    """Return best-of-N time in ms per iteration."""
    times = timeit.repeat(fn, number=iters, repeat=repeats)
    return min(times) * 1000 / iters

def run_benchmark():
    print("Generating test data (seed=42)...")
    dense_data = generate_dense_ips(1000)
    log_data = generate_log_data(1000, 100)
    no_ips_data = generate_no_ips(100_000)
    defanged_data = generate_defanged_data(1000)

    extractor = Extractor()

    inputs = [
        ("Dense IPs (1000, mixed v4+v6)", dense_data, regex_extract_validated),
        ("Sparse Logs (1000 IPs in noise)", log_data, regex_extract_validated),
        ("No IPs (100 KB noise)", no_ips_data, regex_extract_validated),
        ("Defanged IPs (1000, mixed)", defanged_data, regex_defang_extract_validated),
    ]

    # Warmup — let Python JIT/cache settle, Rust side initialize
    for _, data, re_func in inputs:
        re_func(data)
        extractor.extract(data)

    # Disable GC during timed sections to reduce variance
    gc.disable()

    print(f"\nBenchmark: re + ipaddress vs ipextract  (best of {N_REPEATS} x {N_ITERS} iters)")
    print(f"{'Dataset':<36} {'Size':>8} {'re (ms)':>10} {'Rust (ms)':>10} {'Speedup':>9} {'Matches':>9}")
    print("-" * 90)

    for name, data, re_func in inputs:
        size_kb = len(data.encode("utf-8")) / 1024

        # Verify match counts agree
        re_matches = re_func(data)
        rust_matches = extractor.extract(data)
        re_count = len(re_matches)
        rust_count = len(rust_matches)
        count_str = str(rust_count) if re_count == rust_count else f"{re_count}/{rust_count}"

        re_ms = bench(lambda d=data, f=re_func: f(d))
        rust_ms = bench(lambda d=data: extractor.extract(d))
        speedup = re_ms / rust_ms if rust_ms > 0 else float("inf")

        print(f"{name:<36} {size_kb:>6.1f}KB {re_ms:>10.3f} {rust_ms:>10.3f} {speedup:>8.1f}x {count_str:>9}")

    # Unique extraction benchmark
    print(f"\n{'Unique Extraction (1000 IPs, many dupes)':<36}")
    base_ips = [random_ipv4() for _ in range(50)] + [random_ipv6() for _ in range(50)]
    duplicated_data = " ".join(random.choice(base_ips) for _ in range(1000))

    re_unique = regex_extract_unique_validated(duplicated_data)
    rust_unique = extractor.extract_unique(duplicated_data)
    re_count = len(re_unique)
    rust_count = len(rust_unique)
    count_str = str(rust_count) if re_count == rust_count else f"{re_count}/{rust_count}"

    re_ms = bench(lambda: regex_extract_unique_validated(duplicated_data))
    rust_ms = bench(lambda: extractor.extract_unique(duplicated_data))
    speedup = re_ms / rust_ms if rust_ms > 0 else float("inf")

    size_kb = len(duplicated_data.encode("utf-8")) / 1024
    print(f"{'  order-preserving unique':<36} {size_kb:>6.1f}KB {re_ms:>10.3f} {rust_ms:>10.3f} {speedup:>8.1f}x {count_str:>9}")

    gc.enable()

    print("\nNotes:")
    print("  - 're' column = re.findall() + ipaddress.ip_address() validation")
    print("  - 'Rust' column = ipextract.Extractor.extract() (DFA, validates during scan)")
    print("  - Matches column shows re/rust counts if they differ")
    print("  - Speedup = re_time / rust_time (higher is better for Rust)")
    print("  - Times are best-of-5 runs to reduce GC/scheduling noise")

if __name__ == "__main__":
    run_benchmark()
