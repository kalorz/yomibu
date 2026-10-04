#!/usr/bin/env python3
"""Explicit test/example setup; never invoked by the library or ordinary CLI."""

import argparse
import hashlib
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
DESTINATION = Path(__file__).resolve().parent.parent / "target" / "a1"
PREFIX = "sudachi-dictionary-20260723/"
FILES = ["LEGAL", "LICENSE-2.0.txt", "system_core.dic"]


def digest(path):
    checksum = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            checksum.update(chunk)
    return checksum.hexdigest()


def prepare(archive):
    DESTINATION.mkdir(parents=True, exist_ok=True)
    dictionary = DESTINATION / "system_core.dic"
    if archive is None and all((DESTINATION / name).is_file() for name in FILES):
        if dictionary.stat().st_size == DICTIONARY_BYTES and digest(dictionary) == DICTIONARY_SHA256:
            print(f"Pinned dictionary already ready: {dictionary}")
            return
    with tempfile.TemporaryDirectory(prefix=".a1-setup-", dir=DESTINATION.parent) as staging:
        staging = Path(staging)
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
                with bundle.open(info) as source, (staging / name).open("wb") as output:
                    shutil.copyfileobj(source, output)
        staged_dictionary = staging / "system_core.dic"
        if staged_dictionary.stat().st_size != DICTIONARY_BYTES or digest(staged_dictionary) != DICTIONARY_SHA256:
            raise ValueError("Extracted dictionary failed verification")
        # Verify everything first. Keep any usable dictionary until the last replace.
        for name in FILES:
            (staging / name).replace(DESTINATION / name)
    print(f"Pinned dictionary and publisher notices ready: {dictionary}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=Path, help="use an existing pinned ZIP without network")
    arguments = parser.parse_args()
    try:
        prepare(arguments.archive)
    except (OSError, ValueError, KeyError, zipfile.BadZipFile) as error:
        parser.exit(1, f"A1 dictionary setup failed: {error}\n")


if __name__ == "__main__":
    main()
