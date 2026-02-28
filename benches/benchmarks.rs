use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use geoipsed::{ExtractorBuilder, Tag, Tagged};
use std::io::Write;
use std::ops::Range;

// Generate test data for extraction benchmarks
fn generate_ipv4_lines(count: usize) -> Vec<u8> {
    let mut data = Vec::new();
    for i in 0..count {
        // Generate various IPv4 addresses
        let ip = format!(
            "{}.{}.{}.{} - Sample log line {}\n",
            (i % 223) + 1,
            (i * 7) % 256,
            (i * 13) % 256,
            (i * 17) % 256,
            i
        );
        data.extend_from_slice(ip.as_bytes());
    }
    data
}

fn generate_ipv6_lines(count: usize) -> Vec<u8> {
    let mut data = Vec::new();
    for i in 0..count {
        // Generate various IPv6 addresses
        let ip = format!(
            "2001:0db8:{:04x}:{:04x}::{:04x} - Sample log line {}\n",
            (i % 65536),
            ((i * 7) % 65536),
            ((i * 13) % 65536),
            i
        );
        data.extend_from_slice(ip.as_bytes());
    }
    data
}

fn generate_mixed_lines(count: usize) -> Vec<u8> {
    let mut data = Vec::new();
    for i in 0..count {
        if i % 2 == 0 {
            // IPv4 line (Apache-style access log)
            let ip = format!(
                "{}.{}.{}.{} - - [01/Jan/2024:00:00:{:02} +0000] \"GET /index.html HTTP/1.1\" 200 1234\n",
                (i % 223) + 1,
                (i * 7) % 256,
                (i * 13) % 256,
                (i * 17) % 256,
                i % 60
            );
            data.extend_from_slice(ip.as_bytes());
        } else {
            // IPv6 line
            let ip = format!(
                "2001:0db8:{:04x}:{:04x}::{:04x} - - [01/Jan/2024:00:00:{:02} +0000] \"GET /index.html HTTP/1.1\" 200 1234\n",
                (i % 65536),
                ((i * 7) % 65536),
                ((i * 13) % 65536),
                i % 60
            );
            data.extend_from_slice(ip.as_bytes());
        }
    }
    data
}

// Benchmark IPv4 extraction
fn bench_extract_ipv4(c: &mut Criterion) {
    let data = generate_ipv4_lines(1000);
    let extractor = ExtractorBuilder::new()
        .ipv4(true)
        .ipv6(false)
        .private_ips(true)
        .loopback_ips(true)
        .broadcast_ips(true)
        .build()
        .expect("Failed to build extractor");

    let mut group = c.benchmark_group("extract_ipv4");
    group.throughput(Throughput::Bytes(data.len() as u64));

    group.bench_function("find_iter", |b| {
        b.iter(|| {
            let matches: Vec<Range<usize>> = extractor.find_iter(black_box(&data)).collect();
            black_box(matches);
        });
    });

    group.bench_function("match_iter", |b| {
        b.iter(|| {
            extractor.match_iter(black_box(&data)).count()
        });
    });

    group.finish();
}

// Benchmark IPv6 extraction
fn bench_extract_ipv6(c: &mut Criterion) {
    let data = generate_ipv6_lines(1000);
    let extractor = ExtractorBuilder::new()
        .ipv4(false)
        .ipv6(true)
        .private_ips(true)
        .loopback_ips(true)
        .build()
        .expect("Failed to build extractor");

    let mut group = c.benchmark_group("extract_ipv6");
    group.throughput(Throughput::Bytes(data.len() as u64));

    group.bench_function("find_iter", |b| {
        b.iter(|| {
            let matches: Vec<Range<usize>> = extractor.find_iter(black_box(&data)).collect();
            black_box(matches);
        });
    });

    group.bench_function("match_iter", |b| {
        b.iter(|| {
            extractor.match_iter(black_box(&data)).count()
        });
    });

    group.finish();
}

// Benchmark mixed IPv4/IPv6 extraction (realistic workload)
fn bench_extract_mixed(c: &mut Criterion) {
    let data = generate_mixed_lines(1000);
    let extractor = ExtractorBuilder::new()
        .ipv4(true)
        .ipv6(true)
        .private_ips(true)
        .loopback_ips(true)
        .broadcast_ips(true)
        .build()
        .expect("Failed to build extractor");

    let mut group = c.benchmark_group("extract_mixed");
    group.throughput(Throughput::Bytes(data.len() as u64));

    group.bench_function("find_iter", |b| {
        b.iter(|| {
            let matches: Vec<Range<usize>> = extractor.find_iter(black_box(&data)).collect();
            black_box(matches);
        });
    });

    group.bench_function("match_iter", |b| {
        b.iter(|| {
            extractor.match_iter(black_box(&data)).count()
        });
    });

    group.finish();
}

// Benchmark replace_iter for single-pass in-line decoration.
//
// This is the primary API for the geoipsed decoration path: scan input,
// write non-IP gaps unchanged, substitute each IP via callback. The output
// sink is a pre-allocated Vec<u8> to isolate scan+write cost from I/O.
fn bench_replace_iter(c: &mut Criterion) {
    let extractor = ExtractorBuilder::new()
        .ipv4(true)
        .ipv6(true)
        .private_ips(true)
        .loopback_ips(true)
        .broadcast_ips(true)
        .build()
        .expect("Failed to build extractor");

    let mut group = c.benchmark_group("replace_iter");

    // Apache access log: one IP per line, realistic gap ratio
    let mixed_data = generate_mixed_lines(1000);
    group.throughput(Throughput::Bytes(mixed_data.len() as u64));

    // Identity: write each IP unchanged — measures pure scan+gap-write overhead
    group.bench_function("identity", |b| {
        let mut out = Vec::with_capacity(mixed_data.len());
        b.iter(|| {
            out.clear();
            extractor
                .replace_iter(black_box(&mixed_data), &mut out, |m, w| {
                    w.write_all(m.as_bytes())
                })
                .unwrap();
        });
    });

    // Redact: constant replacement — simulates privacy-scrubbing pipelines
    group.bench_function("redact", |b| {
        let mut out = Vec::with_capacity(mixed_data.len());
        b.iter(|| {
            out.clear();
            extractor
                .replace_iter(black_box(&mixed_data), &mut out, |_m, w| {
                    w.write_all(b"[REDACTED]")
                })
                .unwrap();
        });
    });

    // Annotate: write IP + kind tag — simulates lightweight decoration without MMDB
    group.bench_function("annotate", |b| {
        let mut out = Vec::with_capacity(mixed_data.len() * 2);
        b.iter(|| {
            out.clear();
            extractor
                .replace_iter(black_box(&mixed_data), &mut out, |m, w| {
                    write!(w, "<{}>", m.as_str())
                })
                .unwrap();
        });
    });

    group.finish();
}

// Benchmark Tagged.write() with varying number of tags
fn bench_tagged_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("tagged_write");

    // Test line with multiple IP addresses (long enough for 20+ tags)
    let line = b"192.168.1.1 connected to 10.0.0.1 via 172.16.0.1 from 8.8.8.8 through 1.1.1.1 and 93.184.216.34 also 142.250.185.78 plus 151.101.1.140 then 104.16.132.229 more 13.107.42.14 end 192.168.1.15 start 8.8.4.4 here 1.0.0.1 now 8.26.56.26 also 208.67.222.222";

    for tag_count in [1, 2, 5, 20] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_tags", tag_count)),
            &tag_count,
            |b, &tag_count| {
                // Setup: Create a Tagged with the specified number of tags
                let mut tagged = Tagged::new(line);
                let mut pos = 0;
                for i in 0..tag_count {
                    let tag = Tag::new(format!("192.168.1.{}", i))
                        .with_range(pos..pos + 10)
                        .with_decoration(format!("<AS{}_ORG|US|City>", 1000 + i));
                    tagged = tagged.tag(tag);
                    pos += (line.len() - 10) / (tag_count as usize + 1);
                }

                // Benchmark the write operation
                b.iter(|| {
                    let mut output = Vec::with_capacity(1024);
                    tagged.write(black_box(&mut output)).unwrap();
                    black_box(output);
                });
            },
        );
    }

    group.finish();
}

// Benchmark Tagged.write_json() with varying number of tags
fn bench_tagged_json(c: &mut Criterion) {
    let mut group = c.benchmark_group("tagged_json");

    // Test line with multiple IP addresses (long enough for 20+ tags)
    let line = b"192.168.1.1 connected to 10.0.0.1 via 172.16.0.1 from 8.8.8.8 through 1.1.1.1 and 93.184.216.34 also 142.250.185.78 plus 151.101.1.140 then 104.16.132.229 more 13.107.42.14 end 192.168.1.15 start 8.8.4.4 here 1.0.0.1 now 8.26.56.26 also 208.67.222.222";

    for tag_count in [1, 2, 5, 20] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_tags", tag_count)),
            &tag_count,
            |b, &tag_count| {
                // Setup: Create a Tagged with the specified number of tags
                let mut tagged = Tagged::new(line);
                let mut pos = 0;
                for i in 0..tag_count {
                    let tag = Tag::new(format!("192.168.1.{}", i))
                        .with_range(pos..pos + 10)
                        .with_decoration(format!("<AS{}_ORG|US|City>", 1000 + i));
                    tagged = tagged.tag(tag);
                    pos += (line.len() - 10) / (tag_count as usize + 1);
                }

                // Benchmark the write_json operation
                b.iter(|| {
                    let mut output = Vec::with_capacity(1024);
                    let mut tagged_clone = tagged.clone();
                    tagged_clone.write_json(black_box(&mut output)).unwrap();
                    black_box(output);
                });
            },
        );
    }

    group.finish();
}

// Benchmark template rendering via naive string replacement (baseline).
// This mirrors what apply_template in mmdb.rs does today.
fn bench_template_render(c: &mut Criterion) {
    use std::collections::HashMap;

    let template = "<{ip}|AS{asnnum}_{asnorg}|{country_iso}|{city}>";

    let mut values: HashMap<&str, &str> = HashMap::new();
    values.insert("ip", "93.184.216.34");
    values.insert("asnnum", "15133");
    values.insert("asnorg", "EDGECAST");
    values.insert("city", "Los_Angeles");
    values.insert("continent", "NA");
    values.insert("country_iso", "US");
    values.insert("country_full", "United_States");
    values.insert("latitude", "34.0544");
    values.insert("longitude", "-118.2441");
    values.insert("timezone", "America/Los_Angeles");

    c.bench_function("template_render", |b| {
        b.iter(|| {
            let mut result = template.to_string();
            for (key, value) in &values {
                let placeholder = format!("{{{}}}", key);
                result = result.replace(&placeholder, value);
            }
            black_box(result);
        });
    });
}

criterion_group!(
    benches,
    bench_extract_ipv4,
    bench_extract_ipv6,
    bench_extract_mixed,
    bench_replace_iter,
    bench_tagged_write,
    bench_tagged_json,
    bench_template_render
);
criterion_main!(benches);
