from __future__ import annotations

import unittest

from src.math_utils import add, square, subtract


class MathUtilsTests(unittest.TestCase):
    def test_add(self) -> None:
        self.assertEqual(add(2, 3), 5)

    def test_subtract(self) -> None:
        self.assertEqual(subtract(7, 2), 5)

    def test_square(self) -> None:
        self.assertEqual(square(4), 16)


if __name__ == "__main__":
    unittest.main()
