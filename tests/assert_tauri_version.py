"""Assert the Tauri major each consumer fixture resolves.

The default-feature fixture must pick stable v2 even though it never mentions
`tauri`; the v3 fixture must pick the prerelease it opts into. Cargo's `links`
rule means exactly one `tauri` package can exist, so a mixed graph cannot pass.
"""

import argparse
import json
import subprocess


def tauri_versions(manifest):
    output = subprocess.check_output(
        ["cargo", "metadata", "--locked", "--format-version", "1",
         "--manifest-path", str(manifest)],
        # Cargo emits UTF-8; the default text decoding uses the OS locale, which is
        # not UTF-8 on Windows runners.
        encoding="utf-8",
    )
    return sorted(
        package["version"]
        for package in json.loads(output)["packages"]
        if package["name"] == "tauri"
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifest")
    parser.add_argument("major")
    args = parser.parse_args()

    versions = tauri_versions(args.manifest)
    if len(versions) != 1:
        raise SystemExit(f"expected exactly one tauri package, found {versions}")
    if not versions[0].startswith(f"{args.major}."):
        raise SystemExit(
            f"expected tauri {args.major}.x, resolved {versions[0]} from {args.manifest}"
        )
    print(f"{args.manifest}: tauri {versions[0]}")


if __name__ == "__main__":
    main()
