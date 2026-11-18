"""The parity gate must reject drift and missing/empty artifacts."""

from pathlib import Path
import tempfile
import unittest

from compare_bindings import compare


class CompareBindingsTests(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory()
        self.addCleanup(self.scratch.cleanup)
        self.reference = Path(self.scratch.name) / "v2"
        self.candidate = Path(self.scratch.name) / "v3"
        for root in (self.reference, self.candidate):
            root.mkdir()
            (root / "bindings.ts").write_bytes(b"export const value = 1;\n")

    def test_identical_files(self):
        self.assertEqual(compare(self.reference, self.candidate), [])

    def test_changed_binding(self):
        (self.candidate / "bindings.ts").write_bytes(b"export const value = 2;\n")
        self.assertTrue(compare(self.reference, self.candidate))

    def test_missing_binding(self):
        (self.reference / "extra.js").write_bytes(b"export {};\n")
        self.assertTrue(compare(self.reference, self.candidate))

    def test_unexpected_binding(self):
        (self.candidate / "extra.js").write_bytes(b"export {};\n")
        self.assertTrue(compare(self.reference, self.candidate))

    def test_line_endings_are_not_normalized(self):
        (self.candidate / "bindings.ts").write_bytes(b"export const value = 1;\r\n")
        self.assertTrue(compare(self.reference, self.candidate))

    def test_missing_directory(self):
        with self.assertRaisesRegex(ValueError, "Missing bindings"):
            compare(self.reference, self.candidate / "missing")

    def test_empty_candidate(self):
        (self.candidate / "bindings.ts").unlink()
        with self.assertRaisesRegex(ValueError, "Empty bindings"):
            compare(self.reference, self.candidate)

    def test_empty_reference(self):
        (self.reference / "bindings.ts").unlink()
        with self.assertRaisesRegex(ValueError, "Empty bindings"):
            compare(self.reference, self.candidate)


if __name__ == "__main__":
    unittest.main()
