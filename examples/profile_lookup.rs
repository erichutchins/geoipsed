/// Profiling harness: run MMDB lookups in a tight loop for sampling profilers.
///
/// Usage:
///   GEOIP_MMDB_DIR=tests/maxmind cargo run --profile dev-optimized --example profile_lookup
///
///   Then in another terminal:
///     sample <PID> 5 -f /tmp/profile_new.txt
///
///   Or simply run with `time` for a quick measurement.
use geoipsed::{geoip, mmdb};
use std::net::IpAddr;
use termcolor::ColorChoice;

fn main() {
    let mmdb_dir = std::env::var("GEOIP_MMDB_DIR").unwrap_or_else(|_| "tests/maxmind".to_string());

    let mut registry = mmdb::ProviderRegistry::default();
    registry
        .initialize_active_provider(Some(camino::Utf8PathBuf::from(&mmdb_dir)))
        .expect("Failed to init provider");

    let geoipdb = geoip::GeoIPSed::new_with_provider(
        Some(camino::Utf8PathBuf::from(&mmdb_dir)),
        None,
        ColorChoice::Never,
        false,
        registry,
    )
    .expect("Failed to create GeoIPSed");

    let test_ips: Vec<(&str, IpAddr)> = vec![
        ("1.0.0.1", "1.0.0.1".parse().unwrap()),
        ("8.8.8.8", "8.8.8.8".parse().unwrap()),
        ("93.184.216.34", "93.184.216.34".parse().unwrap()),
        ("142.250.185.78", "142.250.185.78".parse().unwrap()),
        ("1.1.1.1", "1.1.1.1".parse().unwrap()),
        ("208.67.222.222", "208.67.222.222".parse().unwrap()),
    ];

    let iterations = 500_000;
    eprintln!("Running {} iterations of MMDB lookups...", iterations);

    let start = std::time::Instant::now();
    for i in 0..iterations {
        let (ip_str, ip) = &test_ips[i % test_ips.len()];
        let result = geoipdb.lookup(*ip, ip_str);
        std::hint::black_box(result);
    }
    let elapsed = start.elapsed();
    eprintln!(
        "Done in {:?} ({:.0} lookups/sec)",
        elapsed,
        iterations as f64 / elapsed.as_secs_f64()
    );
}
