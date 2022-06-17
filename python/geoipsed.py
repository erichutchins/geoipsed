import re
import sys
from functools import lru_cache

from geoip2.database import Reader
from maxminddb import MODE_MMAP_EXT

# regular expressions from cyberchef.io's built-in recipes
IPRE = re.compile(
    """
    (
        (?:(?:\d|[01]?\d\d|2[0-4]\d|25[0-5])\.){3}(?:25[0-5]|2[0-4]\d|[01]?\d\d|\d)
    )
    |
    (
    ((?=.*::)(?!.*::.+::)(::)?([\dA-Fa-f]{1,4}:(:|\b)|){5}|([\dA-Fa-f]{1,4}:){6})((([\dA-Fa-f]{1,4}((?!\3)::|:\b|(?![\dA-Fa-f])))|(?!\2\3)){2}|(((2[0-4]|1\d|[1-9])?\d|25[0-5])\.?\b){4})
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
