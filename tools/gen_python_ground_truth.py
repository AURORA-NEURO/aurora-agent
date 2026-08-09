"""Emits CPython canonical-JSON ground truth used by the bioprism-ids parity tests.

Run: python tools/gen_python_ground_truth.py
The output is pasted into crates/ids/tests/python_parity.rs. Any change to CPython's float
repr or json escaping will show up here as a diff before it can silently break replay.
"""

import json

FLOATS = [
    0.0, -0.0, 1.0, -1.0, 0.5, 100.0,
    1e15, 1e16, 1e17, 1e-4, 1e-5, 1e-6,
    0.1, 0.2, 1 / 3, 1.5, -2.75,
    123456789012345.0, 1234567890123456.0, 12345678901234567.0,
    1e100, 1e-100, 2.220446049250313e-16, 5e-324,
    1.7976931348623157e308, 3.14159265358979, 1e21, 9.99999999999999e15,
    -1e16, -1e-5, 6.02214076e23,
]

STRINGS = [
    "a", '"', "\\", "\n", "\t", "\r", "\b", "\f",
    "\x00", "\x1f", "\x7f", "é", "日本", " ", "a/b",
    " ", "", "😀" if False else "\U0001F600",
]


def main() -> None:
    print("// floats: (input_bits, expected_repr)")
    for v in FLOATS:
        import struct
        bits = struct.unpack("<Q", struct.pack("<d", v))[0]
        print(f'    (0x{bits:016x}u64, {json.dumps(repr(v))}),')

    print()
    print("// strings: (input, expected_canonical_object)")
    for s in STRINGS:
        obj = json.dumps({"k": s}, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
        print(f"    ({json.dumps(s)}, {json.dumps(obj)}),")


if __name__ == "__main__":
    main()
