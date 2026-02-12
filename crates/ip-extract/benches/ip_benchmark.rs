use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use ip_extract::{parse_ipv4_bytes, ExtractorBuilder};
use rand::Rng;
use std::net::{Ipv4Addr, Ipv6Addr};

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

criterion_group!(benches, bench_extraction, bench_ipv4_parser_vs_stdlib);
criterion_main!(benches);
