"""Compare generated binding trees byte-for-byte, including their file sets."""

import argparse
import difflib
from pathlib import Path


def binding_files(root):
    if not root.is_dir():
        raise ValueError(f"Missing bindings directory: {root}")
    files = {path.relative_to(root): path for path in root.rglob("*") if path.is_file()}
    if not files:
        raise ValueError(f"Empty bindings directory: {root}")
    return files


def compare(reference, candidate):
    expected = binding_files(reference)
    actual = binding_files(candidate)
    errors = []
    for name in sorted(expected.keys() - actual.keys()):
        errors.append(f"Missing binding: {candidate / name}")
    for name in sorted(actual.keys() - expected.keys()):
        errors.append(f"Unexpected binding: {candidate / name}")
    for name in sorted(expected.keys() & actual.keys()):
        left = expected[name].read_bytes()
        right = actual[name].read_bytes()
        if left != right:
            errors.append(f"Binding bytes differ: {expected[name]} vs {actual[name]}")
            errors.extend(difflib.unified_diff(
                left.decode("utf-8").splitlines(keepends=True),
                right.decode("utf-8").splitlines(keepends=True),
                fromfile=str(expected[name]), tofile=str(actual[name]),
            ))
    return errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("reference", type=Path)
    parser.add_argument("candidates", type=Path, nargs="+")
    args = parser.parse_args()
    failed = False
    for candidate in args.candidates:
        try:
            errors = compare(args.reference, candidate)
        except ValueError as error:
            errors = [str(error)]
        if errors:
            failed = True
            print("\n".join(errors))
        else:
            print(f"Identical bindings: {args.reference} == {candidate}")
    return int(failed)


if __name__ == "__main__":
    raise SystemExit(main())
