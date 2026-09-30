"""Validate IDS documents against the official IDS 1.0 XSD.

Usage: python validate-ids.py <ids.xsd> <file.ids>...

Needs the `xmlschema` package (`uv run --with xmlschema python ...`). The
XSD is not vendored; point at a local buildingSMART IDS checkout.
Prints one line per invalid file and exits non-zero if any is invalid.
"""

import sys

import xmlschema


def main() -> int:
    schema = xmlschema.XMLSchema(sys.argv[1])
    invalid = 0
    for path in sys.argv[2:]:
        errors = list(schema.iter_errors(path))
        if errors:
            invalid += 1
            print(f"{path}: {errors[0].reason} at {errors[0].path}")
    print(f"{len(sys.argv) - 2 - invalid} valid, {invalid} invalid")
    return 1 if invalid else 0


if __name__ == "__main__":
    sys.exit(main())
