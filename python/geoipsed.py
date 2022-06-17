import sys
import geoip2.database
import re
from functools import lru_cache

IPRE = re.compile("""
    (
        (?:(?:\d|[01]?\d\d|2[0-4]\d|25[0-5])\.){3}(?:25[0-5]|2[0-4]\d|[01]?\d\d|\d)
    )
    |
    (
    ((?=.*::)(?!.*::.+::)(::)?([\dA-Fa-f]{1,4}:(:|\b)|){5}|([\dA-Fa-f]{1,4}:){6})((([\dA-Fa-f]{1,4}((?!\3)::|:\b|(?![\dA-Fa-f])))|(?!\2\3)){2}|(((2[0-4]|1\d|[1-9])?\d|25[0-5])\.?\b){4})
    )""", flags=re.VERBOSE)

citydb = geoip2.database.Reader('/usr/share/GeoIP/GeoLite2-City.mmdb')
asndb = geoip2.database.Reader('/usr/share/GeoIP/GeoLite2-ASN.mmdb')

@lru_cache()
def iplookup(ip):
    return f"<<{ip}>>"

def ipenrich(matchobj):
    return iplookup(matchobj.group(0))

def main():
    for line in sys.stdin:
        enriched = IPRE.sub(ipenrich, line)
        print(enriched, end="")


if __name__ == "__main__":
    sys.exit(main())



