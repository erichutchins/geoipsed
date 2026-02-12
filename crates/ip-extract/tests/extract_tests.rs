use ip_extract::ExtractorBuilder;

/// Simplified test harness to verify IP extraction.
fn check_extraction(
    haystack: &[u8],
    expected: &[&str],
    include_private: bool,
    include_loopback: bool,
) {
    let extractor = ExtractorBuilder::new()
        .ipv4(true)
        .ipv6(true)
        .private_ips(include_private)
        .loopback_ips(include_loopback)
        .build()
        .expect("Failed to build extractor");

    let actual: Vec<String> = extractor
        .find_iter(haystack)
        .map(|range| String::from_utf8_lossy(&haystack[range]).to_string())
        .collect();

    assert_eq!(
        actual,
        expected.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        "\nFailed for haystack: {}\n",
        String::from_utf8_lossy(haystack)
    );
}

#[test]
fn test_ipv4_variations() {
    check_extraction(
        b"Standard: 8.8.8.8, LAN: 192.168.1.1, Loop: 127.0.0.1",
        &["8.8.8.8", "192.168.1.1", "127.0.0.1"],
        true, // include private
        true, // include loopback
    );
}

#[test]
fn test_ipv6_variations() {
    check_extraction(
        b"Full: 2001:0db8:85a3:0000:0000:8a2e:0370:7334, Compressed: 2001:db8::1, Local: ::1",
        &[
            "2001:0db8:85a3:0000:0000:8a2e:0370:7334",
            "2001:db8::1",
            "::1",
        ],
        true,
        true,
    );
}

#[test]
fn test_filtering() {
    let haystack = b"Public: 1.1.1.1, Private: 10.0.0.1, Loopback: 127.0.0.1";

    // Only Public
    check_extraction(haystack, &["1.1.1.1"], false, false);

    // Public + Private
    check_extraction(haystack, &["1.1.1.1", "10.0.0.1"], true, false);

    // Everything
    check_extraction(haystack, &["1.1.1.1", "10.0.0.1", "127.0.0.1"], true, true);
}

#[test]
fn test_delimiters_and_junk() {
    check_extraction(
        b"text 1.1.1.1 text, 2.2.2.2; [3.3.3.3] (8.8.8.8)",
        &["1.1.1.1", "2.2.2.2", "3.3.3.3", "8.8.8.8"],
        true,
        true,
    );
}

#[test]
fn test_invalid_ips() {
    check_extraction(
        b"Not an IP: 256.256.256.256, 1.2.3, 1.2.3.4.5, ::::, 127.0.0.01",
        &[],
        true,
        true,
    );
}

#[test]
fn test_mixed_versions_tight() {
    check_extraction(
        b"1.1.1.1, ::1, 8.8.8.8",
        &["1.1.1.1", "::1", "8.8.8.8"],
        true,
        true,
    );
}
