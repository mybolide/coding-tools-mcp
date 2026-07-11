"""Tiny local pytest shim so the fixture stays dependency-free."""

from __future__ import annotations

import sys
import unittest


def main() -> int:
    start_dir = sys.argv[1] if len(sys.argv) > 1 and not sys.argv[1].startswith("-") else "tests"
    suite = unittest.defaultTestLoader.discover(start_dir)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


if __name__ == "__main__":
    raise SystemExit(main())
