import maxminddb

# This is a test method to make a portable and sharable
# maxmind database file. This file is not used in the production

# Create a new MaxMind database
db = maxminddb.open_database("test.city.mmdb")

# Add some fake IP addresses with geolocation information
db["192.0.2.1"] = {
    "city": {
        "names": {"en": "New York"},
    },
    "country": {
        "iso_code": "US",
        "names": {"en": "United States"},
    },
    "location": {
        "latitude": 40.7128,
        "longitude": -74.0060,
    },
}
db["198.51.100.1"] = {
    "city": {
        "names": {"en": "Los Angeles"},
    },
    "country": {
        "iso_code": "US",
        "names": {"en": "United States"},
    },
    "location": {
        "latitude": 34.0522,
        "longitude": -118.2437,
    },
}
db["203.0.113.1"] = {
    "city": {
        "names": {"en": "Sydney"},
    },
    "country": {
        "iso_code": "AU",
        "names": {"en": "Australia"},
    },
    "location": {
        "latitude": -33.8688,
        "longitude": 151.2093,
    },
}
db["2001:db8::1"] = {
    "city": {
        "names": {"en": "Paris"},
    },
    "country": {
        "iso_code": "FR",
        "names": {"en": "France"},
    },
    "location": {
        "latitude": 48.8566,
        "longitude": 2.3522,
    },
}
db["2001:db8::2"] = {
    "city": {
        "names": {"en": "Tokyo"},
    },
    "country": {
        "iso_code": "JP",
        "names": {"en": "Japan"},
    },
    "location": {
        "latitude": 35.6762,
        "longitude": 139.6503,
    },
}

# Close the database
db.close()

# Create a new MaxMind ASN database
db = maxminddb.open_database("test.asn.mmdb")

# Define ASN data for the fake IP addresses
asn_data = {
    "192.0.2.1": {"autonomous_system_number": 1000, "autonomous_system_organization": "ExampleOrg1"},
    "198.51.100.1": {"autonomous_system_number": 2000, "autonomous_system_organization": "ExampleOrg2"},
    "203.0.113.1": {"autonomous_system_number": 3000, "autonomous_system_organization": "ExampleOrg3"},
    "2001:db8::1": {"autonomous_system_number": 4000, "autonomous_system_organization": "ExampleOrg4"},
    "2001:db8::2": {"autonomous_system_number": 5000, "autonomous_system_organization": "ExampleOrg5"},
}

# Add the ASN data to the database
for ip, data in asn_data.items():
    db[ip] = data

# Close the database
db.close()