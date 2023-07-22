from maxminddb import MODE_MMAP_EXT, InvalidDatabaseError
from geoip2.errors import AddressNotFoundError
from geoip2.database import Reader
from functools import lru_cache
import sys
import re


# ipv4 - copied from cyberchef.org minus the cidr mask
# ipv6 - https://gist.github.com/dfee/6ed3a4b05cfe7a6faf40a2102408d5d8
IPRE = re.compile(
    r"""
    (
        (?:(?:\d|[01]?\d\d|2[0-4]\d|25[0-5])\.){3}(?:25[0-5]|2[0-4]\d|[01]?\d\d|\d)
    )
    |
    (
        (?:(?:(?:(?:[0-9a-fA-F]){1,4}):){1,4}:[^\\s:](?:(?:(?:25[0-5]|(?:2[0-4]|1{0,1}[0-9]){0,1}[0-9])\\.){3,3}(?:25[0-5]|(?:2[0-4]|1{0,1}[0-9]){0,1}[0-9])))|(?:::(?:ffff(?::0{1,4}){0,1}:){0,1}[^\\s:](?:(?:(?:25[0-5]|(?:2[0-4]|1{0,1}[0-9]){0,1}[0-9])\\.){3,3}(?:25[0-5]|(?:2[0-4]|1{0,1}[0-9]){0,1}[0-9])))|(?:fe80:(?::(?:(?:[0-9a-fA-F]){1,4})){0,4}%[0-9a-zA-Z]{1,})|(?::(?:(?::(?:(?:[0-9a-fA-F]){1,4})){1,7}|:))|(?:(?:(?:[0-9a-fA-F]){1,4}):(?:(?::(?:(?:[0-9a-fA-F]){1,4})){1,6}))|(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){1,2}(?::(?:(?:[0-9a-fA-F]){1,4})){1,5})|(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){1,3}(?::(?:(?:[0-9a-fA-F]){1,4})){1,4})|(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){1,4}(?::(?:(?:[0-9a-fA-F]){1,4})){1,3})|(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){1,5}(?::(?:(?:[0-9a-fA-F]){1,4})){1,2})|(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){1,6}:(?:(?:[0-9a-fA-F]){1,4}))|(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){1,7}:)|(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){7,7}(?:(?:[0-9a-fA-F]){1,4}))
    )""",
    flags=re.VERBOSE
)

# Globals are slow, should be made local if possible when comparing performance
citydb = Reader("/usr/share/GeoIP/GeoLite2-City.mmdb", mode=MODE_MMAP_EXT)
asndb = Reader("/usr/share/GeoIP/GeoLite2-ASN.mmdb", mode=MODE_MMAP_EXT)


@lru_cache()
def iplookup(ip: str) -> str:
    try:
        cityrecord = citydb.city(ip)
    except (TypeError, ValueError, AddressNotFoundError, InvalidDatabaseError):
        # no match; do nothing
        return ip

    try:
        asnrecord = asndb.asn(ip)
    except (TypeError, ValueError, AddressNotFoundError, InvalidDatabaseError):
        # also err; do nothing
        return ip

    asnnum = asnrecord.autonomous_system_number
    asnorg = asnrecord.autonomous_system_organization
    isocode = cityrecord.country.iso_code

    return f"""<{ip}|AS{asnnum}_{asnorg}|{isocode}>""".replace(" ", "_")


# Could be a lambda instead
def ipenrich(matchobj: re.Match[str]) -> str:
    return iplookup(matchobj.group(0))


def main():
    for line in sys.stdin:
        enriched = IPRE.sub(ipenrich, line)
        print(enriched, end="")


if __name__ == "__main__":
    sys.exit(main())
