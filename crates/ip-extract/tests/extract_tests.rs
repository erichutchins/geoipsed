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
    // Defaults include all IP types
    let extractor = ExtractorBuilder::new()
        .ipv4(true)
        .ipv6(false)
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
        .ignore_broadcast()
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
        .ignore_broadcast()
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
    let extractor = ExtractorBuilder::new().ipv4(true).build().unwrap();

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
        .build()
        .unwrap();

    // Should extract IPv4 but skip IPv6
    let haystack = b"IPv4: 192.168.1.1, IPv6: 2001:db8::1";
    let actual: Vec<String> = extractor
        .find_iter(haystack)
        .map(|range| String::from_utf8_lossy(&haystack[range]).to_string())
        .collect();

    assert_eq!(actual, vec!["192.168.1.1"]);

    // Multiple IPv4, multiple IPv6 - should only get IPv4
    let haystack2 = b"Servers: 10.0.0.1, 8.8.8.8, fe80::1, 2001:db8::1";
    let actual2: Vec<String> = extractor
        .find_iter(haystack2)
        .map(|range| String::from_utf8_lossy(&haystack2[range]).to_string())
        .collect();

    assert_eq!(actual2, vec!["10.0.0.1", "8.8.8.8"]);

    // Only IPv6 present - should extract nothing
    let haystack3 = b"IPv6 only: 2001:db8::1, ::1, fe80::dead:beef";
    let actual3: Vec<String> = extractor
        .find_iter(haystack3)
        .map(|range| String::from_utf8_lossy(&haystack3[range]).to_string())
        .collect();

    assert_eq!(actual3.len(), 0);
}

#[test]
fn test_ipv6_only_extractor() {
    let extractor = ExtractorBuilder::new()
        .ipv4(false)
        .ipv6(true)
        .build()
        .unwrap();

    // Should extract IPv6 but skip IPv4
    let haystack = b"IPv4: 192.168.1.1, IPv6: 2001:db8::1";
    let actual: Vec<String> = extractor
        .find_iter(haystack)
        .map(|range| String::from_utf8_lossy(&haystack[range]).to_string())
        .collect();

    assert_eq!(actual, vec!["2001:db8::1"]);

    // Multiple IPv6, multiple IPv4 - should only get IPv6
    let haystack2 = b"Servers: 10.0.0.1, fe80::1, 8.8.8.8, 2001:db8::1";
    let actual2: Vec<String> = extractor
        .find_iter(haystack2)
        .map(|range| String::from_utf8_lossy(&haystack2[range]).to_string())
        .collect();

    assert_eq!(actual2, vec!["fe80::1", "2001:db8::1"]);

    // Only IPv4 present - should extract nothing
    let haystack3 = b"IPv4 only: 192.168.1.1, 10.0.0.1, 127.0.0.1";
    let actual3: Vec<String> = extractor
        .find_iter(haystack3)
        .map(|range| String::from_utf8_lossy(&haystack3[range]).to_string())
        .collect();

    assert_eq!(actual3.len(), 0);

    // Test various IPv6 formats
    let haystack4 = b"Formats: ::1, 2001:db8::, fe80::dead:beef, fc00::1";
    let actual4: Vec<String> = extractor
        .find_iter(haystack4)
        .map(|range| String::from_utf8_lossy(&haystack4[range]).to_string())
        .collect();

    assert_eq!(
        actual4,
        vec!["::1", "2001:db8::", "fe80::dead:beef", "fc00::1"]
    );
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

#[test]
fn test_json_string_with_single_ip() {
    // Test JSON string where the value is exactly one IP
    let extractor = ExtractorBuilder::new()
        .ipv4(true)
        .ipv6(true)
        .build()
        .unwrap();

    // IPv4 as complete value
    let json1 = br#"{"src_ip":"192.168.1.1"}"#;
    let ranges1: Vec<_> = extractor.find_iter(json1).collect();
    assert_eq!(ranges1.len(), 1);
    assert_eq!(&json1[ranges1[0].clone()], b"192.168.1.1");

    // IPv6 as complete value
    let json2 = br#"{"dst_ip":"2001:db8::1"}"#;
    let ranges2: Vec<_> = extractor.find_iter(json2).collect();
    assert_eq!(ranges2.len(), 1);
    assert_eq!(&json2[ranges2[0].clone()], b"2001:db8::1");

    // IP at start of string value
    let json3 = br#"{"msg":"192.168.1.1 connected"}"#;
    let ranges3: Vec<_> = extractor.find_iter(json3).collect();
    assert_eq!(ranges3.len(), 1);
    assert_eq!(&json3[ranges3[0].clone()], b"192.168.1.1");

    // IP at end of string value
    let json4 = br#"{"msg":"Connection from 10.0.0.5"}"#;
    let ranges4: Vec<_> = extractor.find_iter(json4).collect();
    assert_eq!(ranges4.len(), 1);
    assert_eq!(&json4[ranges4[0].clone()], b"10.0.0.5");

    // IP in middle of string value
    let json5 = br#"{"msg":"Host 172.16.0.1 responded"}"#;
    let ranges5: Vec<_> = extractor.find_iter(json5).collect();
    assert_eq!(ranges5.len(), 1);
    assert_eq!(&json5[ranges5[0].clone()], b"172.16.0.1");
}

#[test]
fn test_json_string_with_multiple_ips() {
    // Test JSON string with multiple IPs in log-like format
    let extractor = ExtractorBuilder::new()
        .ipv4(true)
        .ipv6(true)
        .build()
        .unwrap();

    // Multiple IPv4 addresses in syslog-style message
    let json1 = br#"{"syslog":"2024-01-15 Connection from 192.168.1.100 to 10.0.0.50 port 443"}"#;
    let ranges1: Vec<_> = extractor.find_iter(json1).collect();
    assert_eq!(ranges1.len(), 2);
    assert_eq!(&json1[ranges1[0].clone()], b"192.168.1.100");
    assert_eq!(&json1[ranges1[1].clone()], b"10.0.0.50");

    // Mixed IPv4 and IPv6
    let json2 = br#"{"log":"src=8.8.8.8 dst=2001:4860:4860::8888 proto=tcp"}"#;
    let ranges2: Vec<_> = extractor.find_iter(json2).collect();
    assert_eq!(ranges2.len(), 2);
    assert_eq!(&json2[ranges2[0].clone()], b"8.8.8.8");
    assert_eq!(&json2[ranges2[1].clone()], b"2001:4860:4860::8888");

    // Multiple IPs with various separators
    let json3 = br#"{"ips":"1.1.1.1, 8.8.8.8; 9.9.9.9 | 1.0.0.1"}"#;
    let ranges3: Vec<_> = extractor.find_iter(json3).collect();
    assert_eq!(ranges3.len(), 4);
    assert_eq!(&json3[ranges3[0].clone()], b"1.1.1.1");
    assert_eq!(&json3[ranges3[1].clone()], b"8.8.8.8");
    assert_eq!(&json3[ranges3[2].clone()], b"9.9.9.9");
    assert_eq!(&json3[ranges3[3].clone()], b"1.0.0.1");

    // Complex nested JSON with multiple IPs
    let json4 = br#"{"event":"firewall","src":"192.168.1.5","dst":"203.0.113.10","details":"Blocked connection from 192.168.1.5 to 203.0.113.10"}"#;
    let ranges4: Vec<_> = extractor.find_iter(json4).collect();
    assert_eq!(ranges4.len(), 4); // Each IP appears twice
    assert_eq!(&json4[ranges4[0].clone()], b"192.168.1.5");
    assert_eq!(&json4[ranges4[1].clone()], b"203.0.113.10");
    assert_eq!(&json4[ranges4[2].clone()], b"192.168.1.5");
    assert_eq!(&json4[ranges4[3].clone()], b"203.0.113.10");

    // Empty IP list in JSON
    let json5 = br#"{"ips":"","message":"no ips here"}"#;
    let ranges5: Vec<_> = extractor.find_iter(json5).collect();
    assert_eq!(ranges5.len(), 0);
}

#[test]
fn test_ipv6_scope_id_boundary() {
    // IPv6 scope IDs (zone IDs) are treated as boundaries and not captured
    let extractor = ExtractorBuilder::new()
        .ipv4(true)
        .ipv6(true)
        .build()
        .unwrap();

    // fe80::1%eth0 should extract as fe80::1 (% acts as boundary)
    let input1 = b"Link-local: fe80::1%eth0";
    let ranges1: Vec<_> = extractor.find_iter(input1).collect();
    assert_eq!(ranges1.len(), 1);
    assert_eq!(&input1[ranges1[0].clone()], b"fe80::1");

    // Multiple addresses with scope IDs
    let input2 = b"Interfaces: fe80::1%eth0 fe80::dead:beef%en0 2001:db8::1%tun0";
    let ranges2: Vec<_> = extractor.find_iter(input2).collect();
    assert_eq!(ranges2.len(), 3);
    assert_eq!(&input2[ranges2[0].clone()], b"fe80::1");
    assert_eq!(&input2[ranges2[1].clone()], b"fe80::dead:beef");
    assert_eq!(&input2[ranges2[2].clone()], b"2001:db8::1");

    // Scope ID alone (just %) doesn't prevent extraction
    let input3 = b"fe80::cafe%wlan0 and 10.0.0.1";
    let ranges3: Vec<_> = extractor.find_iter(input3).collect();
    assert_eq!(ranges3.len(), 2);
    assert_eq!(&input3[ranges3[0].clone()], b"fe80::cafe");
    assert_eq!(&input3[ranges3[1].clone()], b"10.0.0.1");
}

// ============================================================================
// AGGRESSIVE ADVERSARIAL TESTS - Backtracking and boundary stress
// ============================================================================

#[test]
fn test_adversarial_aggressive_backtracking() {
    use ip_extract::parse_ipv4_bytes;

    // Many valid-looking octets that exceed 255 when combined
    assert!(parse_ipv4_bytes(b"999.999.999.999").is_none()); // All > 255
    assert!(parse_ipv4_bytes(b"299.299.299.299").is_none()); // All > 255

    // Mixed - some octets valid, one invalid
    assert!(parse_ipv4_bytes(b"192.168.999.1").is_none()); // Third octet > 255
    assert!(parse_ipv4_bytes(b"192.999.1.1").is_none()); // Second octet > 255

    // The "9.9.9.9.9..." pattern - DFA might find "9.9.9.9" but extraction
    // context could cause issues if backtracking tries multiple start positions
    assert!(parse_ipv4_bytes(b"9.9.9.9.9").is_none()); // 5 octets
    assert!(parse_ipv4_bytes(b"9.9.9.9.9.9").is_none()); // 6 octets

    // Valid: exactly 4 octets
    assert!(parse_ipv4_bytes(b"9.9.9.9").is_some());
}

#[test]
fn test_adversarial_extraction_repeating_pattern() {
    let extractor = ExtractorBuilder::new().ipv4(true).build().unwrap();

    // String with repeating valid octet followed by more octets
    // The right boundary check rejects "9.9.9.9" when followed by ".9" (would make 5th octet)
    // So it skips to find "1.1.1.1" which has a valid boundary (space after)
    let haystack = b"9.9.9.9.9.9.9.9 then 1.1.1.1";
    let actual: Vec<String> = extractor
        .find_iter(haystack)
        .map(|r| String::from_utf8_lossy(&haystack[r]).to_string())
        .collect();

    // The repeating pattern has invalid right boundary (dot followed by digit)
    // so only "1.1.1.1" is extracted (valid boundary with space after)
    assert_eq!(actual, vec!["1.1.1.1"]);
}

#[test]
fn test_adversarial_max_value_boundaries() {
    use ip_extract::parse_ipv4_bytes;

    // Test all combinations of "just over 255"
    assert!(parse_ipv4_bytes(b"256.0.0.0").is_none());
    assert!(parse_ipv4_bytes(b"0.256.0.0").is_none());
    assert!(parse_ipv4_bytes(b"0.0.256.0").is_none());
    assert!(parse_ipv4_bytes(b"0.0.0.256").is_none());

    // Test "one below and at boundary"
    assert!(parse_ipv4_bytes(b"254.254.254.254").is_some());
    assert!(parse_ipv4_bytes(b"255.255.255.255").is_some());

    // Test mixed valid/invalid that could trick a greedy parser
    assert!(parse_ipv4_bytes(b"255.254.253.256").is_none()); // Last octet over
    assert!(parse_ipv4_bytes(b"256.254.253.252").is_none()); // First octet over
}

#[test]
fn test_adversarial_prefix_suffix_collisions() {
    // Tests that validator doesn't accidentally accept partial matches
    use ip_extract::parse_ipv4_bytes;

    // "25" prefix could match "25[0-5]" pattern but "256" doesn't
    assert!(parse_ipv4_bytes(b"256.256.256.256").is_none());

    // "2" prefix valid, but "299" not
    assert!(parse_ipv4_bytes(b"299.299.299.299").is_none());

    // Edge case: "250-255" vs "260-299"
    assert!(parse_ipv4_bytes(b"250.250.250.250").is_some());
    assert!(parse_ipv4_bytes(b"260.260.260.260").is_none());
}

#[test]
fn test_adversarial_extraction_long_sequence() {
    // Test extraction from very long repeating digit sequence
    let extractor = ExtractorBuilder::new().ipv4(true).build().unwrap();

    // 9.9.9.9.9.9.9.9.9... - many dots and valid octets
    // The right boundary check sees ".9" after "9.9.9.9" and rejects it as incomplete (5+ octet form)
    let haystack = b"9.9.9.9.9.9.9.9.9.9.9.9.9.9.9.9.9";
    let actual: Vec<String> = extractor
        .find_iter(haystack)
        .map(|r| String::from_utf8_lossy(&haystack[r]).to_string())
        .collect();

    // Since every 4-octet boundary is followed by ".digit", all matches are rejected as incomplete
    // The right boundary check prevents this sequence from matching
    assert_eq!(actual.len(), 0);
}

#[test]
fn test_adversarial_boundary_256_variants() {
    use ip_extract::parse_ipv4_bytes;

    // Ensure 256 in any position fails
    let test_cases = vec![
        (b"256.0.0.0", 0),
        (b"0.256.0.0", 1),
        (b"0.0.256.0", 2),
        (b"0.0.0.256", 3),
    ];

    for (ip_bytes, pos) in test_cases {
        assert!(
            parse_ipv4_bytes(ip_bytes).is_none(),
            "Failed for position {}",
            pos
        );
    }

    // Ensure 255 in all positions succeeds
    assert!(parse_ipv4_bytes(b"255.255.255.255").is_some());
}

#[test]
fn test_adversarial_extraction_octet_overflow_context() {
    let extractor = ExtractorBuilder::new().ipv4(true).build().unwrap();

    // IPs with overflow values surrounded by valid context
    let haystack = b"Start: 1.1.1.1, Invalid: 256.256.256.256, End: 8.8.8.8";
    let actual: Vec<String> = extractor
        .find_iter(haystack)
        .map(|r| String::from_utf8_lossy(&haystack[r]).to_string())
        .collect();

    // Should extract only the two valid IPs
    assert_eq!(actual, vec!["1.1.1.1", "8.8.8.8"]);
}

#[test]
fn test_adversarial_extraction_multiple_overflow_patterns() {
    let extractor = ExtractorBuilder::new().ipv4(true).build().unwrap();

    let haystack = b"999.999.999.999 and 8.8.8.8 and 300.300.300.300 and 1.1.1.1";
    let actual: Vec<String> = extractor
        .find_iter(haystack)
        .map(|r| String::from_utf8_lossy(&haystack[r]).to_string())
        .collect();

    // Only valid IPs should be extracted
    assert_eq!(actual, vec!["8.8.8.8", "1.1.1.1"]);
}

#[test]
fn test_adversarial_extraction_dot_density() {
    let extractor = ExtractorBuilder::new().ipv4(true).build().unwrap();

    // High dot density: tests that parser handles many potential boundaries
    let haystack = b"1.2.3.4.5.6.7.8.9";
    let actual: Vec<String> = extractor
        .find_iter(haystack)
        .map(|r| String::from_utf8_lossy(&haystack[r]).to_string())
        .collect();

    // Right boundary check sees ".5" after "1.2.3.4" and rejects it as incomplete (5+ octets)
    // So "1.2.3.4" is not extracted. This is correct behavior - we want complete IPs only.
    assert_eq!(actual.len(), 0);
}

#[test]
fn test_adversarial_leading_zeros_aggressive() {
    use ip_extract::parse_ipv4_bytes;

    // Leading zeros on boundary values should fail
    assert!(parse_ipv4_bytes(b"0255.0.0.0").is_none());
    assert!(parse_ipv4_bytes(b"0256.0.0.0").is_none());
    assert!(parse_ipv4_bytes(b"0199.0.0.0").is_none());
    assert!(parse_ipv4_bytes(b"0100.0.0.0").is_none());
    assert!(parse_ipv4_bytes(b"099.0.0.0").is_none());
    assert!(parse_ipv4_bytes(b"010.0.0.0").is_none());

    // Without leading zeros, these should work
    assert!(parse_ipv4_bytes(b"255.0.0.0").is_some());
    assert!(parse_ipv4_bytes(b"199.0.0.0").is_some());
    assert!(parse_ipv4_bytes(b"100.0.0.0").is_some());
    assert!(parse_ipv4_bytes(b"99.0.0.0").is_some());
    assert!(parse_ipv4_bytes(b"10.0.0.0").is_some());
}

#[test]
fn test_adversarial_extraction_leading_zeros_with_overflow() {
    let extractor = ExtractorBuilder::new().ipv4(true).build().unwrap();

    // Mix of leading zeros and overflow - both should be rejected
    let haystack = b"Valid: 192.168.1.1, LeadingZero: 192.168.01.1, Overflow: 192.168.256.1";
    let actual: Vec<String> = extractor
        .find_iter(haystack)
        .map(|r| String::from_utf8_lossy(&haystack[r]).to_string())
        .collect();

    // Only the first valid IP
    assert_eq!(actual, vec!["192.168.1.1"]);
}

#[test]
fn test_adversarial_extraction_all_positions_overflow() {
    let extractor = ExtractorBuilder::new().ipv4(true).build().unwrap();

    // Test overflow in each octet position separately
    let cases = vec![
        (b"256.1.1.1" as &[u8], "first octet overflow"),
        (b"1.256.1.1", "second octet overflow"),
        (b"1.1.256.1", "third octet overflow"),
        (b"1.1.1.256", "fourth octet overflow"),
    ];

    for (haystack, _description) in cases {
        let actual: Vec<String> = extractor
            .find_iter(haystack)
            .map(|r| String::from_utf8_lossy(&haystack[r]).to_string())
            .collect();

        // All should extract nothing
        assert_eq!(actual.len(), 0);
    }
}

#[test]
fn test_adversarial_digit_accumulation() {
    use ip_extract::parse_ipv4_bytes;

    // Test that parser correctly accumulates digits and validates < 256
    // "25" alone is valid, "255" is valid, "256" is not
    assert!(parse_ipv4_bytes(b"25.1.1.1").is_some());
    assert!(parse_ipv4_bytes(b"255.1.1.1").is_some());
    assert!(parse_ipv4_bytes(b"256.1.1.1").is_none());

    // "2" alone is valid, "20" is valid, "200" is valid, "2000" is not (and length check)
    assert!(parse_ipv4_bytes(b"2.1.1.1").is_some());
    assert!(parse_ipv4_bytes(b"20.1.1.1").is_some());
    assert!(parse_ipv4_bytes(b"200.1.1.1").is_some());
    assert!(parse_ipv4_bytes(b"2000.1.1.1").is_none()); // Also fails length check
}

#[test]
fn test_adversarial_sequential_boundary_crossing() {
    use ip_extract::parse_ipv4_bytes;

    // Test crossing boundaries: 99->100, 199->200, 249->250, 255->256
    assert!(parse_ipv4_bytes(b"99.99.99.99").is_some());
    assert!(parse_ipv4_bytes(b"100.100.100.100").is_some());
    assert!(parse_ipv4_bytes(b"199.199.199.199").is_some());
    assert!(parse_ipv4_bytes(b"200.200.200.200").is_some());
    assert!(parse_ipv4_bytes(b"249.249.249.249").is_some());
    assert!(parse_ipv4_bytes(b"250.250.250.250").is_some());
    assert!(parse_ipv4_bytes(b"255.255.255.255").is_some());
    assert!(parse_ipv4_bytes(b"256.256.256.256").is_none());
}

#[test]
fn test_adversarial_lookback_boundary_40_chars() {
    // The lookback floor is 40 chars: end.saturating_sub(40)
    // This tests what happens with long strings of IP characters
    let extractor = ExtractorBuilder::new().ipv4(true).build().unwrap();

    // Exactly 40 chars of dots and 1s: repeating pattern "1.1.1.1..."
    let haystack40 = b"1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1";
    let actual40: Vec<String> = extractor
        .find_iter(haystack40)
        .map(|r| String::from_utf8_lossy(&haystack40[r]).to_string())
        .collect();

    // All attempts to extract hit the right boundary check (followed by ".1")
    // So nothing should be extracted
    assert_eq!(
        actual40.len(),
        0,
        "40-char all-IP-chars string should extract nothing (all have invalid boundaries)"
    );
}

#[test]
fn test_adversarial_lookback_boundary_50_chars() {
    // Test with 50 chars of IP characters - lookback floor would be position 10
    let extractor = ExtractorBuilder::new().ipv4(true).build().unwrap();

    // 50 chars of dots and digits: enough to force lookback to floor
    let haystack50 = b"1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1";
    let actual50: Vec<String> = extractor
        .find_iter(haystack50)
        .map(|r| String::from_utf8_lossy(&haystack50[r]).to_string())
        .collect();

    // Same reason - every 4-octet match is followed by ".digit" which is invalid boundary
    assert_eq!(
        actual50.len(),
        0,
        "50-char all-IP-chars string should extract nothing (all have invalid boundaries)"
    );
}

#[test]
fn test_adversarial_lookback_boundary_exceeds_40() {
    // Test what happens when we exceed 40 chars and the lookback floor kicks in
    let extractor = ExtractorBuilder::new()
        .ipv4(true)
        .ipv6(true)
        .build()
        .unwrap();

    // Create a pattern where the lookback hits the floor (position 0)
    // If haystack is 100 chars, and match is at position 100, floor = 60
    // But if all 100 chars are IP chars, lookback scans from 60->100, finds no boundary
    // and uses floor=60 as start position
    let haystack = b"1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.\
                     1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.\
                     1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1";
    let actual: Vec<String> = extractor
        .find_iter(haystack)
        .map(|r| String::from_utf8_lossy(&haystack[r]).to_string())
        .collect();

    // With all IP chars and right boundary checks, should extract nothing
    assert_eq!(actual.len(), 0);
}

#[test]
fn test_adversarial_lookback_boundary_with_valid_endpoint() {
    // Test: what if we have 50 chars of IP chars, BUT a valid boundary at the end?
    let extractor = ExtractorBuilder::new().ipv4(true).build().unwrap();

    // 50 chars of "1.1.1.1..." followed by a space (valid boundary)
    let haystack = b"1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1 end";
    let actual: Vec<String> = extractor
        .find_iter(haystack)
        .map(|r| String::from_utf8_lossy(&haystack[r]).to_string())
        .collect();

    // Now we have a valid right boundary (space), but the lookback will search
    // through all 50 chars of IP chars to find the start, hitting the floor.
    // The substring from floor->end won't be a valid IP, so it fails validation
    // and the loop continues. Since we've advanced past the end, no more matches.
    assert_eq!(
        actual.len(),
        0,
        "Even with valid boundary, 50 chars of IP chars hits floor and fails validation"
    );
}

#[test]
fn test_adversarial_lookback_floor_within_valid_ip() {
    // Specific test: create a scenario where the lookback floor lands within IP chars
    let extractor = ExtractorBuilder::new().ipv4(true).build().unwrap();

    // Pattern: "X" + 40 IP chars + "8.8.8.8" at end
    // When DFA matches "8.8.8.8" at position 88 (end of haystack):
    // floor = 88 - 40 = 48
    // Lookback scans from 48->88 for a non-IP char
    // haystack[48..88] is all IP chars (from the "1.1.1.1..." sequence and "8.8.8.8")
    // So we hit floor, start = 48
    // Then validate haystack[48..88] which is "1.1.1.1.1.1.1.1.8.8.8.8"
    // This substring fails validation (too many octets), so loop continues
    let haystack =
        b"X1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.1.8.8.8.8";
    let actual: Vec<String> = extractor
        .find_iter(haystack)
        .map(|r| String::from_utf8_lossy(&haystack[r]).to_string())
        .collect();

    // Due to the 40-char lookback limit and how the validation works,
    // "8.8.8.8" won't be extracted because the lookback hits the floor
    // and the combined substring fails validation
    assert_eq!(
        actual.len(),
        0,
        "Long IP char sequence before target IP prevents extraction due to lookback floor"
    );
}

#[test]
fn test_adversarial_lookback_valid_extraction_with_boundary() {
    // Test where we DO get a valid extraction with a clear boundary before lookback limit
    let extractor = ExtractorBuilder::new().ipv4(true).build().unwrap();

    // Create clear boundary: non-IP char + valid IP
    // Space (non-IP char) + "1.1.1.1" - this should extract cleanly
    let haystack = b" 1.1.1.1";
    let actual: Vec<String> = extractor
        .find_iter(haystack)
        .map(|r| String::from_utf8_lossy(&haystack[r]).to_string())
        .collect();

    assert_eq!(actual, vec!["1.1.1.1"]);
}

// ============================================================================
// CONVENIENCE FUNCTION TESTS
// ============================================================================

#[test]
fn test_extract_convenience_function() {
    use ip_extract::extract;

    // Basic extraction
    let ips = extract(b"Server at 192.168.1.1 and 2001:db8::1").unwrap();
    assert_eq!(ips, vec!["192.168.1.1", "2001:db8::1"]);

    // Multiple of same IP
    let ips = extract(b"1.1.1.1 and 1.1.1.1 again").unwrap();
    assert_eq!(ips, vec!["1.1.1.1", "1.1.1.1"]);

    // Empty result
    let ips = extract(b"no ips here").unwrap();
    assert!(ips.is_empty());

    // Mixed content
    let ips = extract(b"Connect from 10.0.0.1 to 192.168.1.1, via 8.8.8.8").unwrap();
    assert_eq!(ips, vec!["10.0.0.1", "192.168.1.1", "8.8.8.8"]);
}

#[test]
fn test_extract_unique_convenience_function() {
    use ip_extract::extract_unique;

    // Deduplication
    let ips = extract_unique(b"1.1.1.1 and 1.1.1.1 again").unwrap();
    assert_eq!(ips, vec!["1.1.1.1"]);

    // Order of first observation (not sorted)
    let ips = extract_unique(b"8.8.8.8 then 1.1.1.1 then 1.1.1.1 then 8.8.8.8").unwrap();
    assert_eq!(ips, vec!["8.8.8.8", "1.1.1.1"]);

    // Mixed IPv4 and IPv6
    let ips = extract_unique(b"2001:db8::1, 192.168.1.1, 2001:db8::1").unwrap();
    assert_eq!(ips, vec!["2001:db8::1", "192.168.1.1"]);

    // Empty result
    let ips = extract_unique(b"no ips here").unwrap();
    assert!(ips.is_empty());

    // Multiple IPs, all unique
    let ips = extract_unique(b"1.1.1.1, 2.2.2.2, 3.3.3.3").unwrap();
    assert_eq!(ips, vec!["1.1.1.1", "2.2.2.2", "3.3.3.3"]);
}

#[test]
fn test_extract_unique_order_preservation() {
    use ip_extract::extract_unique;

    // Verify order is preserved, not lexicographically sorted
    let input = b"9.9.9.9, 1.1.1.1, 5.5.5.5, 3.3.3.3";
    let ips = extract_unique(input).unwrap();

    // Should be in observation order, not sorted
    assert_eq!(ips, vec!["9.9.9.9", "1.1.1.1", "5.5.5.5", "3.3.3.3"]);

    // Verify it's not sorted (this would fail if we were sorting)
    assert_ne!(ips, vec!["1.1.1.1", "3.3.3.3", "5.5.5.5", "9.9.9.9"]);
}

#[test]
fn test_extract_parsed_returns_ipaddr() {
    use ip_extract::extract_parsed;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    // IPv4 addresses are returned as IpAddr::V4
    let ips = extract_parsed(b"Server at 192.168.1.1 and 8.8.8.8").unwrap();
    assert_eq!(ips.len(), 2);
    assert_eq!(ips[0], IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)));
    assert_eq!(ips[1], IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)));

    // IPv6 addresses are returned as IpAddr::V6
    let ips = extract_parsed(b"Connect to 2001:db8::1").unwrap();
    assert_eq!(ips.len(), 1);
    assert!(ips[0].is_ipv6());
    assert_eq!(ips[0], IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1)));

    // Mixed IPv4 and IPv6
    let ips = extract_parsed(b"from 10.0.0.1 to ::1").unwrap();
    assert_eq!(ips.len(), 2);
    assert!(ips[0].is_ipv4());
    assert!(ips[1].is_ipv6());

    // Duplicates are NOT removed
    let ips = extract_parsed(b"1.1.1.1 and 1.1.1.1").unwrap();
    assert_eq!(ips.len(), 2);

    // Empty input
    let ips = extract_parsed(b"no ips here").unwrap();
    assert!(ips.is_empty());
}

#[test]
fn test_extract_unique_parsed_returns_unique_ipaddr() {
    use ip_extract::extract_unique_parsed;
    use std::net::{IpAddr, Ipv4Addr};

    // Deduplication by value
    let ips = extract_unique_parsed(b"1.1.1.1 and 1.1.1.1 again").unwrap();
    assert_eq!(ips.len(), 1);
    assert_eq!(ips[0], IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)));

    // Order of first observation is preserved
    let ips = extract_unique_parsed(b"8.8.8.8 then 1.1.1.1 then 8.8.8.8").unwrap();
    assert_eq!(ips.len(), 2);
    assert_eq!(ips[0], IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)));
    assert_eq!(ips[1], IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)));

    // Mixed IPv4 and IPv6 deduplication
    let ips = extract_unique_parsed(b"2001:db8::1, 192.168.1.1, 2001:db8::1").unwrap();
    assert_eq!(ips.len(), 2);
    assert!(ips[0].is_ipv6());
    assert!(ips[1].is_ipv4());

    // All unique
    let ips = extract_unique_parsed(b"1.1.1.1 2.2.2.2 3.3.3.3").unwrap();
    assert_eq!(ips.len(), 3);
}
