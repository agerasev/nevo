#!/usr/bin/env python3
"""Compare seeded traces across revisions without modifying the checkout."""

import argparse
import hashlib
import io
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile


HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
FILES = ("Cargo.toml", "Cargo.lock", "src")


def snapshot(destination, revision):
    if revision is None:
        for name in FILES:
            source = REPO / name
            if source.is_dir():
                shutil.copytree(source, destination / name)
            else:
                shutil.copy2(source, destination / name)
        label = "working tree"
    else:
        label = subprocess.check_output(
            ["git", "rev-parse", "--verify", "--end-of-options", revision + "^{commit}"],
            cwd=REPO,
            text=True,
        ).strip()
        archive = subprocess.check_output(
            ["git", "archive", label, *FILES], cwd=REPO
        )
        with tarfile.open(fileobj=io.BytesIO(archive)) as contents:
            contents.extractall(destination, filter="data")
    (destination / "wgame").symlink_to(REPO / "wgame", target_is_directory=True)
    shutil.copy2(HERE / "trace.rs", destination / "src/bin/trait_trace.rs")
    return label


def capture(root, name, revision):
    source = root / name
    source.mkdir()
    label = snapshot(source, revision)
    output = root / (name + ".txt")
    print(f"{name}: {label}", flush=True)
    with output.open("w") as stream:
        subprocess.run(
            [
                "cargo", "run", "--locked", "--offline", "--no-default-features",
                "--bin", "trait_trace", "--manifest-path", str(source / "Cargo.toml"),
                "--target-dir", str(root / "target"),
            ],
            cwd=source,
            stdout=stream,
            check=True,
        )
    result = output.read_bytes()
    print(f"{name}: {len(result)} bytes; SHA256 {hashlib.sha256(result).hexdigest()}")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--before", default="f585839", help="original Git revision")
    parser.add_argument("--after", help="candidate Git revision; default: working tree")
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="nevo-trait-parity.") as temporary:
        root = Path(temporary)
        before = capture(root, "before", args.before)
        after = capture(root, "after", args.after)
        if before != after:
            raise SystemExit("DIFFER: seeded traces changed")
    print("MATCH: debug/baseline × seeds 1/42, initial state and 128 ticks each")


if __name__ == "__main__":
    main()
