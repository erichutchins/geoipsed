use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use ip_extract::{parse_ipv4_bytes, ExtractorBuilder};
use rand::Rng;
use std::io::Write;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

// Helper function to generate a random IPv4 address
fn random_ipv4() -> String {
    let mut rng = rand::thread_rng();
    let ip = Ipv4Addr::new(rng.gen(), rng.gen(), rng.gen(), rng.gen());
    ip.to_string()
}

// Helper function to generate a random IPv6 address
fn random_ipv6() -> String {
    let mut rng = rand::thread_rng();
    let seg: [u16; 8] = rng.gen();
    let ip = Ipv6Addr::new(
        seg[0], seg[1], seg[2], seg[3], seg[4], seg[5], seg[6], seg[7],
    );
    ip.to_string()
}

// Generate a dense string of IPs (just IPs separated by spaces/newlines)
fn generate_dense_ips(n: usize) -> Vec<u8> {
    let mut s = String::new();
    for i in 0..n {
        if i % 2 == 0 {
            s.push_str(&random_ipv4());
        } else {
            s.push_str(&random_ipv6());
        }
        s.push_str(if i % 10 == 0 { "\n" } else { " " });
    }
    s.into_bytes()
}

// Generate a sparse string of IPs (simulating logs)
// We'll intersperse real IPs with some "noise" text.
fn generate_log_data(n_ips: usize, bytes_per_ip: usize) -> Vec<u8> {
    let mut rng = rand::thread_rng();
    let mut s = String::new();

    // Some typical log-like noise
    let noise_chars: Vec<char> =
        "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-:[]/."
            .chars()
            .collect();

    for i in 0..n_ips {
        // Add some random noise
        let noise_len = rng.gen_range(bytes_per_ip / 2..bytes_per_ip * 2);
        for _ in 0..noise_len {
            s.push(noise_chars[rng.gen_range(0..noise_chars.len())]);
        }

        s.push(' ');
        if i % 2 == 0 {
            s.push_str(&random_ipv4());
        } else {
            s.push_str(&random_ipv6());
        }
        s.push(' ');
    }
    s.into_bytes()
}

// Generate text with NO IPs at all, to measure pure scanning overhead
fn generate_no_ips(size: usize) -> Vec<u8> {
    let mut rng = rand::thread_rng();
    let noise_chars: Vec<char> =
        "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-:[]/."
            .chars()
            .collect();
    let mut s = String::with_capacity(size);
    for _ in 0..size {
        s.push(noise_chars[rng.gen_range(0..noise_chars.len())]);
    }
    s.into_bytes()
}

fn bench_extraction(c: &mut Criterion) {
    let extractor = ExtractorBuilder::new()
        .ipv4(true)
        .ipv6(true)
        .build()
        .unwrap();

    let mut group = c.benchmark_group("ip_extraction");

    // Benchmark 1: Dense IPs
    // About 1000 IPs ~ 20-30KB depending on format
    let dense_input = generate_dense_ips(1000);
    group.throughput(Throughput::Bytes(dense_input.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("dense_ips", dense_input.len()),
        &dense_input,
        |b, input| {
            b.iter(|| {
                // Count matches to force iteration
                extractor.find_iter(input).count()
            });
        },
    );

    // Benchmark 2: Sparse logs
    // 1000 IPs spread out with ~100 bytes of noise per IP => ~100KB total
    let log_input = generate_log_data(1000, 100);
    group.throughput(Throughput::Bytes(log_input.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("sparse_logs", log_input.len()),
        &log_input,
        |b, input| {
            b.iter(|| extractor.find_iter(input).count());
        },
    );

    // Benchmark 3: No IPs (scanning overhead)
    // 100KB of pure text, zero matches
    let clean_input = generate_no_ips(100_000);
    group.throughput(Throughput::Bytes(clean_input.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("no_ips", clean_input.len()),
        &clean_input,
        |b, input| {
            b.iter(|| extractor.find_iter(input).count());
        },
    );

    group.finish();
}

fn bench_ipv4_parser_vs_stdlib(c: &mut Criterion) {
    let mut group = c.benchmark_group("ipv4_parser");

    // Generate test IPv4 addresses as byte slices
    let test_ips: Vec<&[u8]> = vec![
        b"192.168.1.1",
        b"10.0.0.1",
        b"8.8.8.8",
        b"172.16.0.1",
        b"255.255.255.255",
        b"0.0.0.0",
        b"127.0.0.1",
    ];

    // Benchmark our hand-optimized parser
    group.bench_function("hand_optimized_parse_ipv4_bytes", |b| {
        b.iter(|| {
            for ip_bytes in &test_ips {
                let _ = parse_ipv4_bytes(ip_bytes);
            }
        });
    });

    // Benchmark stdlib parser (convert to string first, then parse)
    group.bench_function("stdlib_ipv4addr_parse", |b| {
        b.iter(|| {
            for ip_bytes in &test_ips {
                let ip_str = std::str::from_utf8(ip_bytes).unwrap();
                let _ = ip_str.parse::<Ipv4Addr>();
            }
        });
    });

    // Benchmark stdlib parser (direct from bytes - allocates String)
    group.bench_function("stdlib_ipv4addr_parse_with_string_alloc", |b| {
        b.iter(|| {
            for ip_bytes in &test_ips {
                let ip_string = String::from_utf8_lossy(ip_bytes);
                let _ = ip_string.parse::<Ipv4Addr>();
            }
        });
    });

    group.finish();
}

/// Compare `match_iter` and `find_iter` on identical inputs.
///
/// `match_iter` wraps each result in `IpMatch` (bytes + range + kind) — this
/// measures whether that wrapper has any throughput cost vs bare `Range<usize>`.
fn bench_match_iter_vs_find_iter(c: &mut Criterion) {
    let extractor = ExtractorBuilder::new()
        .ipv4(true)
        .ipv6(true)
        .build()
        .unwrap();

    let mut group = c.benchmark_group("match_iter_vs_find_iter");

    let dense_input = generate_dense_ips(1000);
    group.throughput(Throughput::Bytes(dense_input.len() as u64));

    group.bench_with_input(
        BenchmarkId::new("find_iter", dense_input.len()),
        &dense_input,
        |b, input| {
            b.iter(|| extractor.find_iter(input).count());
        },
    );

    group.bench_with_input(
        BenchmarkId::new("match_iter", dense_input.len()),
        &dense_input,
        |b, input| {
            b.iter(|| extractor.match_iter(input).count());
        },
    );

    let log_input = generate_log_data(1000, 100);
    group.throughput(Throughput::Bytes(log_input.len() as u64));

    group.bench_with_input(
        BenchmarkId::new("find_iter_logs", log_input.len()),
        &log_input,
        |b, input| {
            b.iter(|| extractor.find_iter(input).count());
        },
    );

    group.bench_with_input(
        BenchmarkId::new("match_iter_logs", log_input.len()),
        &log_input,
        |b, input| {
            b.iter(|| extractor.match_iter(input).count());
        },
    );

    group.finish();
}

/// Benchmark `replace_iter` for single-pass in-place decoration.
///
/// Simulates the geoipsed decoration path: scan line, write gaps unchanged,
/// write a substitution for each IP. Measures the full round-trip including
/// output writes, using a `Vec<u8>` sink to isolate from I/O overhead.
fn bench_replace_iter(c: &mut Criterion) {
    let extractor = ExtractorBuilder::new()
        .ipv4(true)
        .ipv6(true)
        .build()
        .unwrap();

    let mut group = c.benchmark_group("replace_iter");

    // Dense: mostly IPs, few gaps
    let dense_input = generate_dense_ips(1000);
    group.throughput(Throughput::Bytes(dense_input.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("identity_dense", dense_input.len()),
        &dense_input,
        |b, input| {
            let mut out = Vec::with_capacity(input.len());
            b.iter(|| {
                out.clear();
                extractor
                    .replace_iter(input, &mut out, |m, w| w.write_all(m.as_bytes()))
                    .unwrap()
            });
        },
    );

    // Sparse logs: realistic decoration workload
    let log_input = generate_log_data(1000, 100);
    group.throughput(Throughput::Bytes(log_input.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("identity_logs", log_input.len()),
        &log_input,
        |b, input| {
            let mut out = Vec::with_capacity(input.len());
            b.iter(|| {
                out.clear();
                extractor
                    .replace_iter(input, &mut out, |m, w| w.write_all(m.as_bytes()))
                    .unwrap()
            });
        },
    );

    // Redaction: write fixed replacement per match (variable-length output)
    group.bench_with_input(
        BenchmarkId::new("redact_logs", log_input.len()),
        &log_input,
        |b, input| {
            let mut out = Vec::with_capacity(input.len());
            b.iter(|| {
                out.clear();
                extractor
                    .replace_iter(input, &mut out, |_m, w| w.write_all(b"[REDACTED]"))
                    .unwrap()
            });
        },
    );

    group.finish();
}

/// Benchmark `IpMatch::ip()` vs the old `str.parse::<IpAddr>()` approach.
///
/// The old path parsed every match via `str.parse::<IpAddr>()` which tries
/// IPv4 first then IPv6 (two attempts for IPv6 matches). `IpMatch::ip()` uses
/// the known `IpKind` discriminant to dispatch directly.
fn bench_ip_parsing(c: &mut Criterion) {
    let extractor = ExtractorBuilder::new()
        .ipv4(true)
        .ipv6(true)
        .build()
        .unwrap();

    let mut group = c.benchmark_group("ip_parsing");

    let mixed_input = generate_log_data(500, 50);

    // New path: match_iter provides kind, ip() dispatches directly
    group.bench_function("match_iter_ip_direct", |b| {
        b.iter(|| {
            extractor
                .match_iter(&mixed_input)
                .map(|m| m.ip())
                .collect::<Vec<IpAddr>>()
        });
    });

    // Old path: find_iter returns Range, caller parses via str.parse (trial-and-error)
    group.bench_function("find_iter_str_parse", |b| {
        b.iter(|| {
            extractor
                .find_iter(&mixed_input)
                .filter_map(|r| {
                    std::str::from_utf8(&mixed_input[r])
                        .ok()
                        .and_then(|s| s.parse::<IpAddr>().ok())
                })
                .collect::<Vec<IpAddr>>()
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_extraction,
    bench_ipv4_parser_vs_stdlib,
    bench_match_iter_vs_find_iter,
    bench_replace_iter,
    bench_ip_parsing,
);
criterion_main!(benches);
