"""Tests for the ipextract Python package.

Run after `maturin develop` from crates/ipextract-py/.
"""
import pytest
import ipextract


# ---------------------------------------------------------------------------
# Module-level convenience functions
# ---------------------------------------------------------------------------

def test_extract_returns_list_of_strings():
    result = ipextract.extract("Connect from 192.168.1.1 to 8.8.8.8")
    assert result == ["192.168.1.1", "8.8.8.8"]


def test_extract_ipv6():
    result = ipextract.extract("server at 2001:db8::1")
    assert result == ["2001:db8::1"]


def test_extract_empty_returns_empty_list():
    assert ipextract.extract("no ips here") == []


def test_extract_unique_deduplicates():
    result = ipextract.extract_unique("1.2.3.4 1.2.3.4 5.6.7.8")
    assert result == ["1.2.3.4", "5.6.7.8"]


def test_extract_unique_preserves_first_seen_order():
    result = ipextract.extract_unique("5.6.7.8 1.2.3.4 5.6.7.8")
    assert result == ["5.6.7.8", "1.2.3.4"]


def test_extract_bytes_input():
    result = ipextract.extract(b"host 10.0.0.1 connected")
    assert result == ["10.0.0.1"]


# ---------------------------------------------------------------------------
# Extractor default constructor
# ---------------------------------------------------------------------------

def test_default_extractor_includes_private():
    e = ipextract.Extractor()
    assert e.extract("192.168.1.1") == ["192.168.1.1"]


def test_default_extractor_includes_loopback():
    e = ipextract.Extractor()
    assert e.extract("127.0.0.1") == ["127.0.0.1"]


def test_default_extractor_accepts_bytes():
    e = ipextract.Extractor()
    assert e.extract(b"host 10.0.0.1") == ["10.0.0.1"]


# ---------------------------------------------------------------------------
# Constructor kwargs
# ---------------------------------------------------------------------------

def test_constructor_private_false_excludes_private():
    e = ipextract.Extractor(private=False)
    assert e.extract("192.168.1.1 8.8.8.8") == ["8.8.8.8"]


def test_constructor_loopback_false_excludes_loopback():
    e = ipextract.Extractor(loopback=False)
    assert e.extract("127.0.0.1 1.1.1.1") == ["1.1.1.1"]


def test_constructor_ipv6_false_skips_ipv6():
    e = ipextract.Extractor(ipv6=False)
    result = e.extract("1.2.3.4 and 2001:db8::1")
    assert "2001:db8::1" not in result
    assert "1.2.3.4" in result


def test_constructor_ipv4_false_skips_ipv4():
    e = ipextract.Extractor(ipv4=False)
    result = e.extract("1.2.3.4 and 2001:db8::1")
    assert "1.2.3.4" not in result
    assert "2001:db8::1" in result


def test_constructor_no_ip_versions_raises():
    with pytest.raises(ValueError):
        ipextract.Extractor(ipv4=False, ipv6=False)


# ---------------------------------------------------------------------------
# Fluent builder methods
# ---------------------------------------------------------------------------

def test_only_public_excludes_private():
    e = ipextract.Extractor().only_public()
    assert e.extract("192.168.1.1 8.8.8.8") == ["8.8.8.8"]


def test_only_public_excludes_loopback():
    e = ipextract.Extractor().only_public()
    assert e.extract("127.0.0.1 1.1.1.1") == ["1.1.1.1"]


def test_ignore_private_method():
    e = ipextract.Extractor().ignore_private()
    assert e.extract("10.0.0.1 8.8.8.8") == ["8.8.8.8"]


def test_ignore_loopback_method():
    e = ipextract.Extractor().ignore_loopback()
    assert e.extract("127.0.0.1 1.1.1.1") == ["1.1.1.1"]


def test_fluent_chaining():
    e = ipextract.Extractor().ignore_private().ignore_loopback()
    assert e.extract("192.168.1.1 127.0.0.1 8.8.8.8") == ["8.8.8.8"]


def test_fluent_ipv4_false():
    e = ipextract.Extractor().ipv4(False)
    result = e.extract("1.2.3.4 2001:db8::1")
    assert "1.2.3.4" not in result


def test_fluent_returns_new_object():
    """Fluent methods must not mutate the original."""
    base = ipextract.Extractor()
    public = base.only_public()
    # base still includes private
    assert base.extract("192.168.1.1") == ["192.168.1.1"]
    # public does not
    assert public.extract("192.168.1.1") == []


# ---------------------------------------------------------------------------
# extract_with_offsets
# ---------------------------------------------------------------------------

def test_extract_with_offsets_basic():
    text = "host 1.2.3.4 end"
    result = ipextract.Extractor().extract_with_offsets(text)
    assert len(result) == 1
    ip, start, end = result[0]
    assert ip == "1.2.3.4"
    assert text[start:end] == "1.2.3.4"


def test_extract_with_offsets_multiple():
    text = "a 1.1.1.1 b 2.2.2.2 c"
    result = ipextract.Extractor().extract_with_offsets(text)
    assert len(result) == 2
    for ip, start, end in result:
        assert text[start:end] == ip


def test_extract_with_offsets_bytes():
    data = b"host 10.0.0.1 port 80"
    result = ipextract.Extractor().extract_with_offsets(data)
    assert len(result) == 1
    ip, start, end = result[0]
    assert ip == "10.0.0.1"
    assert data[start:end] == b"10.0.0.1"


# ---------------------------------------------------------------------------
# Extractor reuse (key for performance)
# ---------------------------------------------------------------------------

def test_extractor_is_reusable():
    e = ipextract.Extractor().only_public()
    assert e.extract("1.1.1.1") == ["1.1.1.1"]
    assert e.extract("8.8.8.8") == ["8.8.8.8"]
    assert e.extract("192.168.1.1") == []
