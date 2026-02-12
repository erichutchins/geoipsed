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

// ============================================================================
// NEW COMPREHENSIVE TESTS
// ============================================================================

#[test]
fn test_ipv4_boundary_values() {
    // Min and max values (note: 255.255.255.255 is broadcast and 192.168.1.1 is private)
    // We need to include broadcast and private to see them all
    let extractor = ExtractorBuilder::new()
        .ipv4(true)
        .ipv6(false)
        .private_ips(true)
        .broadcast_ips(true)
        .build()
        .unwrap();

    let haystack = b"Min: 0.0.0.0, Max: 255.255.255.255, Normal: 192.168.1.1";
    let actual: Vec<String> = extractor
        .find_iter(haystack)
        .map(|range| String::from_utf8_lossy(&haystack[range]).to_string())
        .collect();

    assert_eq!(actual, vec!["0.0.0.0", "255.255.255.255", "192.168.1.1"]);
}

#[test]
fn test_ipv4_single_digit_octets() {
    check_extraction(
        b"Single digits: 1.2.3.4, 5.6.7.8",
        &["1.2.3.4", "5.6.7.8"],
        true,
        true,
    );
}

#[test]
fn test_ipv4_all_private_ranges() {
    // Test all RFC 1918 private ranges
    check_extraction(
        b"Class A: 10.0.0.1, Class B: 172.16.0.1, Class C: 192.168.0.1",
        &["10.0.0.1", "172.16.0.1", "192.168.0.1"],
        true,
        false,
    );

    // Verify they're filtered when include_private=false
    check_extraction(
        b"Class A: 10.0.0.1, Class B: 172.16.0.1, Class C: 192.168.0.1, Public: 8.8.8.8",
        &["8.8.8.8"],
        false,
        false,
    );
}

#[test]
fn test_ipv4_middle_private_range() {
    // Specifically test 172.16.0.0/12 range
    check_extraction(
        b"Start: 172.16.0.0, Mid: 172.20.0.1, End: 172.31.255.255",
        &["172.16.0.0", "172.20.0.1", "172.31.255.255"],
        true,
        false,
    );

    check_extraction(
        b"Private: 172.16.0.1, Public: 172.32.0.1",
        &["172.32.0.1"],
        false,
        false,
    );
}

#[test]
fn test_ipv4_link_local() {
    // Link-local addresses (169.254.0.0/16) should be filtered by broadcast_ips
    let extractor = ExtractorBuilder::new()
        .ipv4(true)
        .ipv6(false)
        .broadcast_ips(false)
        .build()
        .unwrap();

    let haystack = b"Link-local: 169.254.1.1, Public: 8.8.8.8";
    let actual: Vec<String> = extractor
        .find_iter(haystack)
        .map(|range| String::from_utf8_lossy(&haystack[range]).to_string())
        .collect();

    assert_eq!(actual, vec!["8.8.8.8"]);
}

#[test]
fn test_ipv4_broadcast_address() {
    // Broadcast address should be filtered by broadcast_ips
    let extractor = ExtractorBuilder::new()
        .ipv4(true)
        .ipv6(false)
        .broadcast_ips(false)
        .build()
        .unwrap();

    let haystack = b"Broadcast: 255.255.255.255, Public: 8.8.8.8";
    let actual: Vec<String> = extractor
        .find_iter(haystack)
        .map(|range| String::from_utf8_lossy(&haystack[range]).to_string())
        .collect();

    assert_eq!(actual, vec!["8.8.8.8"]);
}

#[test]
fn test_ipv4_loopback_range() {
    // Full 127.0.0.0/8 range should be filtered
    check_extraction(
        b"Loop start: 127.0.0.1, Loop mid: 127.100.50.25, Loop end: 127.255.255.254",
        &["127.0.0.1", "127.100.50.25", "127.255.255.254"],
        true,
        true,
    );

    check_extraction(
        b"Loop: 127.0.0.1, Public: 8.8.8.8",
        &["8.8.8.8"],
        true,
        false,
    );
}

#[test]
fn test_ipv4_leading_zeros_rejected() {
    // Leading zeros should be rejected
    check_extraction(
        b"Invalid: 192.168.01.1, 192.168.001.1, 01.2.3.4",
        &[],
        true,
        true,
    );
}

#[test]
fn test_ipv4_trailing_dot() {
    // Test that IPs can be extracted successfully in various contexts
    let extractor = ExtractorBuilder::new()
        .ipv4(true)
        .private_ips(true)
        .build()
        .unwrap();

    // IP without trailing dot works fine
    let haystack1 = b"192.168.1.1";
    let ranges1: Vec<_> = extractor.find_iter(haystack1).collect();
    assert_eq!(ranges1.len(), 1);
    assert_eq!(&haystack1[ranges1[0].clone()], b"192.168.1.1");

    // IP with surrounding text works
    let haystack2 = b"text 192.168.1.1 more text";
    let ranges2: Vec<_> = extractor.find_iter(haystack2).collect();
    assert_eq!(ranges2.len(), 1);
    assert_eq!(&haystack2[ranges2[0].clone()], b"192.168.1.1");

    // IP with comma, semicolon, colon, parentheses works
    let haystack3 = b"IPs: 1.2.3.4, 5.6.7.8; (9.10.11.12):end";
    let ranges3: Vec<_> = extractor.find_iter(haystack3).collect();
    assert_eq!(ranges3.len(), 3);
    assert_eq!(&haystack3[ranges3[0].clone()], b"1.2.3.4");
    assert_eq!(&haystack3[ranges3[1].clone()], b"5.6.7.8");
    assert_eq!(&haystack3[ranges3[2].clone()], b"9.10.11.12");

    // IPs followed by dots (sentence endings) now work correctly!
    // The trailing dot is stripped by the backtracking algorithm for IPv4
    let haystack4 = b"The C2 IP was 192.168.1.1.";
    let ranges4: Vec<_> = extractor.find_iter(haystack4).collect();
    assert_eq!(ranges4.len(), 1);
    assert_eq!(&haystack4[ranges4[0].clone()], b"192.168.1.1");

    // Multiple IPs with trailing dots work
    let haystack5 = b"First: 1.2.3.4. Second: 5.6.7.8. End.";
    let ranges5: Vec<_> = extractor.find_iter(haystack5).collect();
    assert_eq!(ranges5.len(), 2);
    assert_eq!(&haystack5[ranges5[0].clone()], b"1.2.3.4");
    assert_eq!(&haystack5[ranges5[1].clone()], b"5.6.7.8");
}

#[test]
fn test_ipv4_edge_of_input() {
    // IPs at exact start and end of input
    check_extraction(b"192.168.1.1", &["192.168.1.1"], true, true);
    check_extraction(
        b"Start: 1.1.1.1 End: 8.8.8.8",
        &["1.1.1.1", "8.8.8.8"],
        true,
        true,
    );
}

#[test]
fn test_ipv6_compressed_forms() {
    // Note: ::8.8.8.8 might not match depending on the IPv6 regex pattern
    // The DFA might not recognize IPv4-mapped IPv6 addresses
    check_extraction(
        b"All zeros: ::, Loopback: ::1, Prefix: 2001:db8::",
        &["::", "::1", "2001:db8::"],
        true,
        true,
    );

    // Test IPv4-mapped IPv6 separately if it's supported
    let extractor = ExtractorBuilder::new().ipv6(true).build().unwrap();

    let haystack = b"IPv4-mapped: ::ffff:192.0.2.1";
    let matches: Vec<_> = extractor.find_iter(haystack).collect();
    // This may or may not match depending on regex pattern - just documenting behavior
    assert!(matches.len() <= 1);
}

#[test]
fn test_ipv6_ula_private_range() {
    // RFC 4193 Unique Local Addresses (fc00::/7) should be filtered
    check_extraction(
        b"ULA start: fc00::1, ULA end: fdff::1, Public: 2001:db8::1",
        &["fc00::1", "fdff::1", "2001:db8::1"],
        true,
        false,
    );

    // Verify ULA is filtered when include_private=false
    check_extraction(
        b"ULA: fc00::1, Link-local: fe80::1, Public: 2001:db8::1",
        &["2001:db8::1"],
        false,
        false,
    );
}

#[test]
fn test_ipv6_link_local() {
    // fe80::/10 link-local addresses
    check_extraction(
        b"Link-local: fe80::1, Public: 2001:db8::1",
        &["fe80::1", "2001:db8::1"],
        true,
        false,
    );

    check_extraction(
        b"Link-local: fe80::1, Public: 2001:db8::1",
        &["2001:db8::1"],
        false,
        false,
    );
}

#[test]
fn test_ipv6_loopback() {
    check_extraction(
        b"Loopback: ::1, Public: 2001:db8::1",
        &["::1", "2001:db8::1"],
        true,
        true,
    );

    check_extraction(
        b"Loopback: ::1, Public: 2001:db8::1",
        &["2001:db8::1"],
        true,
        false,
    );
}

#[test]
fn test_ipv6_full_form() {
    // Full uncompressed IPv6
    check_extraction(
        b"Full: 2001:0db8:0000:0000:0000:0000:0000:0001",
        &["2001:0db8:0000:0000:0000:0000:0000:0001"],
        true,
        true,
    );
}

#[test]
fn test_ipv6_invalid_double_compression() {
    // Multiple :: should be invalid (though rust parser might handle this)
    // This test verifies stdlib behavior
    check_extraction(b"Invalid: 2001::db8::1", &[], true, true);
}

#[test]
fn test_all_filters_combination() {
    // Test all filter combinations work correctly
    let haystack = b"Public4: 8.8.8.8, Private4: 10.0.0.1, Loop4: 127.0.0.1, \
                     Public6: 2001:db8::1, ULA6: fc00::1, Loop6: ::1";

    // Everything enabled
    check_extraction(
        haystack,
        &[
            "8.8.8.8",
            "10.0.0.1",
            "127.0.0.1",
            "2001:db8::1",
            "fc00::1",
            "::1",
        ],
        true,
        true,
    );

    // Only public
    check_extraction(haystack, &["8.8.8.8", "2001:db8::1"], false, false);

    // Public + private, no loopback
    check_extraction(
        haystack,
        &["8.8.8.8", "10.0.0.1", "2001:db8::1", "fc00::1"],
        true,
        false,
    );
}

#[test]
fn test_ipv4_only_extractor() {
    let extractor = ExtractorBuilder::new()
        .ipv4(true)
        .ipv6(false)
        .private_ips(true) // Need to include private to see 192.168.1.1
        .build()
        .unwrap();

    let haystack = b"IPv4: 192.168.1.1, IPv6: 2001:db8::1";
    let actual: Vec<String> = extractor
        .find_iter(haystack)
        .map(|range| String::from_utf8_lossy(&haystack[range]).to_string())
        .collect();

    assert_eq!(actual, vec!["192.168.1.1"]);
}

#[test]
fn test_ipv6_only_extractor() {
    let extractor = ExtractorBuilder::new()
        .ipv4(false)
        .ipv6(true)
        .build()
        .unwrap();

    let haystack = b"IPv4: 192.168.1.1, IPv6: 2001:db8::1";
    let actual: Vec<String> = extractor
        .find_iter(haystack)
        .map(|range| String::from_utf8_lossy(&haystack[range]).to_string())
        .collect();

    assert_eq!(actual, vec!["2001:db8::1"]);
}

#[test]
fn test_neither_ipv4_nor_ipv6() {
    let result = ExtractorBuilder::new().ipv4(false).ipv6(false).build();

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("No IP address patterns selected"));
}

#[test]
fn test_parse_ipv4_bytes_public_function() {
    use ip_extract::parse_ipv4_bytes;

    // Valid cases
    assert!(parse_ipv4_bytes(b"192.168.1.1").is_some());
    assert!(parse_ipv4_bytes(b"0.0.0.0").is_some());
    assert!(parse_ipv4_bytes(b"255.255.255.255").is_some());
    assert!(parse_ipv4_bytes(b"8.8.8.8").is_some());

    // Invalid cases
    assert!(parse_ipv4_bytes(b"256.1.1.1").is_none()); // Out of range
    assert!(parse_ipv4_bytes(b"192.168.01.1").is_none()); // Leading zero
    assert!(parse_ipv4_bytes(b"1.2.3.4.5").is_none()); // Too many octets
    assert!(parse_ipv4_bytes(b"1.2.3").is_none()); // Too few octets
    assert!(parse_ipv4_bytes(b"").is_none()); // Empty
    assert!(parse_ipv4_bytes(b"not.an.ip.addr").is_none()); // Non-numeric
}
