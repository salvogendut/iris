#!/usr/bin/env python3
"""Convert a PROM image to a Rust byte array without changing its bytes."""

import argparse
import hashlib
from pathlib import Path
import re


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("image", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("symbol", help="Rust static name (uppercase letters, digits and underscores)")
    args = parser.parse_args()
    if not re.fullmatch(r"[A-Z][A-Z0-9_]*", args.symbol):
        parser.error("invalid Rust static name")
    data = args.image.read_bytes()
    if not data:
        parser.error("the PROM image is empty")
    if args.image.resolve() == args.output.resolve():
        parser.error("output must differ from the source image")
    with args.output.open("w", encoding="ascii", newline="\n") as output:
        output.write(f"// Generated from {args.image.name} with tools/embed-prom.py\n")
        output.write(f"// Size: {len(data)} bytes\n// SHA-1: {hashlib.sha1(data).hexdigest()}\n\n")
        output.write(f"#[rustfmt::skip]\npub static {args.symbol}: [u8; {len(data)}] = [\n")
        for offset in range(0, len(data), 16):
            output.write("    " + " ".join(f"0x{byte:02X}," for byte in data[offset:offset + 16]) + "\n")
        output.write("];\n")


if __name__ == "__main__":
    main()
