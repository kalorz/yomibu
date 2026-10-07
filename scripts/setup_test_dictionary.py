#!/usr/bin/env python3
"""Explicit test/example setup; never invoked by the library or ordinary CLI."""

import argparse
import hashlib
import os
from pathlib import Path
import shutil
import tempfile
import time
import urllib.request
import zipfile

URL = "https://sudachi.s3.ap-northeast-1.amazonaws.com/sudachidict/sudachi-dictionary-20260723-core.zip"
ARCHIVE_SHA256 = "b6e835f63440f97474c2da45d80950f73746e632e40bbfc168b4041729135e1f"
DICTIONARY_SHA256 = "53fa281d11eef3769712fe1c3c892117338f9892bee6daf4dad51daa5281bb6f"
ARCHIVE_BYTES = 72_276_502
DICTIONARY_BYTES = 217_466_039
DESTINATION = Path(__file__).resolve().parent.parent / "target" / "test-resources" / "sudachi-core"
PREFIX = "sudachi-dictionary-20260723/"
FILES = {
    "LEGAL": (6037, "725a8776b38e058b185e905594bc9a2437dbf3787df022fffeefedb9a84e4665"),
    "LICENSE-2.0.txt": (11358, "cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30"),
    "system_core.dic": (DICTIONARY_BYTES, DICTIONARY_SHA256),
}


class DurabilityUncertain(OSError):
    """The complete bundle is visible, but syncing its publication failed."""


def digest(path):
    checksum = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            checksum.update(chunk)
    return checksum.hexdigest()


def verified_bundle(directory):
    return all(
        (directory / name).is_file()
        and (directory / name).stat().st_size == size
        and digest(directory / name) == checksum
        for name, (size, checksum) in FILES.items()
    )


def sync_directory(directory):
    descriptor = os.open(directory, os.O_RDONLY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def prepare(archive):
    # Persist newly created ancestor entries before publishing into this directory.
    missing_parents = []
    directory = DESTINATION
    while not directory.exists():
        missing_parents.append(directory.parent)
        directory = directory.parent
    DESTINATION.mkdir(parents=True, exist_ok=True)
    for parent in reversed(missing_parents):
        sync_directory(parent)
    current = DESTINATION / "current"
    dictionary = current / "system_core.dic"
    if archive is None and verified_bundle(current.resolve()):
        print(f"Pinned dictionary already ready: {dictionary}")
        return
    with tempfile.TemporaryDirectory(prefix=".sudachi-setup-", dir=DESTINATION) as staging:
        staging = Path(staging)
        contents = staging / "bundle"
        contents.mkdir()
        if archive is None:
            archive = staging / "dictionary.zip"
            deadline = time.monotonic() + 300
            size = 0
            with urllib.request.urlopen(URL, timeout=30) as response, archive.open("wb") as output:
                while chunk := response.read(1024 * 1024):
                    size += len(chunk)
                    if size > ARCHIVE_BYTES or time.monotonic() > deadline:
                        raise ValueError("Dictionary download exceeded size or time limit")
                    output.write(chunk)
        if archive.stat().st_size != ARCHIVE_BYTES or digest(archive) != ARCHIVE_SHA256:
            raise ValueError("Archive does not match the pinned publisher download")
        with zipfile.ZipFile(archive) as bundle:
            for name in FILES:
                # Extract only these exact members; never trust paths in an archive.
                info = bundle.getinfo(PREFIX + name)
                limit = DICTIONARY_BYTES if name == "system_core.dic" else 1024 * 1024
                if info.file_size > limit:
                    raise ValueError(f"Oversized archive member: {name}")
                with bundle.open(info) as source, (contents / name).open("wb") as output:
                    shutil.copyfileobj(source, output)
                    output.flush()
                    os.fsync(output.fileno())
        if not verified_bundle(contents):
            raise ValueError("Extracted bundle failed verification")
        # Setup never edits or removes completed bundles; readers may still use them.
        sync_directory(contents)
        bundle_path = DESTINATION / (".bundle-" + staging.name[len(".sudachi-setup-"):])
        contents.rename(bundle_path)
        sync_directory(DESTINATION)
        pointer = staging / "current"
        pointer.symlink_to(bundle_path.name, target_is_directory=True)
        pointer.replace(current)
        try:
            sync_directory(DESTINATION)
        except OSError as error:
            raise DurabilityUncertain(
                "Complete bundle is visible, but its durability is uncertain after publication."
            ) from error
    print(f"Pinned dictionary and publisher notices ready: {dictionary}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=Path, help="use an existing pinned ZIP without network")
    arguments = parser.parse_args()
    try:
        prepare(arguments.archive)
    except (OSError, ValueError, KeyError, zipfile.BadZipFile) as error:
        parser.exit(1, f"Sudachi dictionary setup failed: {error}\n")


if __name__ == "__main__":
    main()
