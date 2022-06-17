import re
import sys
from functools import lru_cache

from geoip2.database import Reader
from maxminddb import MODE_MMAP_EXT

# ipv4 - copied from cyberchef.org minus the cidr mask
# ipv6 - https://gist.github.com/dfee/6ed3a4b05cfe7a6faf40a2102408d5d8
IPRE = re.compile(
    """
    (
        (?:(?:\d|[01]?\d\d|2[0-4]\d|25[0-5])\.){3}(?:25[0-5]|2[0-4]\d|[01]?\d\d|\d)
    )
    |
    (
        (?:(?:(?:(?:[0-9a-fA-F]){1,4}):){1,4}:[^\\s:](?:(?:(?:25[0-5]|(?:2[0-4]|1{0,1}[0-9]){0,1}[0-9])\\.){3,3}(?:25[0-5]|(?:2[0-4]|1{0,1}[0-9]){0,1}[0-9])))|(?:::(?:ffff(?::0{1,4}){0,1}:){0,1}[^\\s:](?:(?:(?:25[0-5]|(?:2[0-4]|1{0,1}[0-9]){0,1}[0-9])\\.){3,3}(?:25[0-5]|(?:2[0-4]|1{0,1}[0-9]){0,1}[0-9])))|(?:fe80:(?::(?:(?:[0-9a-fA-F]){1,4})){0,4}%[0-9a-zA-Z]{1,})|(?::(?:(?::(?:(?:[0-9a-fA-F]){1,4})){1,7}|:))|(?:(?:(?:[0-9a-fA-F]){1,4}):(?:(?::(?:(?:[0-9a-fA-F]){1,4})){1,6}))|(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){1,2}(?::(?:(?:[0-9a-fA-F]){1,4})){1,5})|(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){1,3}(?::(?:(?:[0-9a-fA-F]){1,4})){1,4})|(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){1,4}(?::(?:(?:[0-9a-fA-F]){1,4})){1,3})|(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){1,5}(?::(?:(?:[0-9a-fA-F]){1,4})){1,2})|(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){1,6}:(?:(?:[0-9a-fA-F]){1,4}))|(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){1,7}:)|(?:(?:(?:(?:[0-9a-fA-F]){1,4}):){7,7}(?:(?:[0-9a-fA-F]){1,4}))
    )""",
    flags=re.VERBOSE,
)

citydb = Reader("/usr/share/GeoIP/GeoLite2-City.mmdb", mode=MODE_MMAP_EXT)
asndb = Reader("/usr/share/GeoIP/GeoLite2-ASN.mmdb", mode=MODE_MMAP_EXT)


@lru_cache()
def iplookup(ip):
    try:
        cityrecord = citydb.city(ip)
    except:
        # no match; do nothing
        return ip

    try:
        asnrecord = asndb.asn(ip)
    except:
        # also err; do nothing
        return ip

    asnnum = asnrecord.autonomous_system_number
    asnorg = asnrecord.autonomous_system_organization
    isocode = cityrecord.country.iso_code

    return f"""<{ip}|AS{asnnum}_{asnorg}|{isocode}>""".replace(" ", "_")


def ipenrich(matchobj):
    return iplookup(matchobj.group(0))


def main():
    for line in sys.stdin:
        enriched = IPRE.sub(ipenrich, line)
        print(enriched, end="")


if __name__ == "__main__":
    sys.exit(main())
