"""Offline installer boundary tests with tiny synthetic archives and private directories."""

import contextlib
import hashlib
import importlib.util
import io
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock
import zipfile


class DictionarySetupTests(unittest.TestCase):
    def setUp(self):
        spec = importlib.util.spec_from_file_location(
            "setup_test_dictionary", Path(__file__).with_name("setup_test_dictionary.py")
        )
        self.setup = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.setup)
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.destination = self.root / "installed"
        self.setup.DESTINATION = self.destination
        self.files = {
            "LEGAL": b"synthetic legal notice",
            "LICENSE-2.0.txt": b"synthetic license notice",
            "system_core.dic": b"synthetic dictionary",
        }
        self.archive = self.root / "fixture.zip"
        self.configure_archive()

    def configure_archive(self):
        with zipfile.ZipFile(self.archive, "w") as bundle:
            for name, data in self.files.items():
                bundle.writestr(self.setup.PREFIX + name, data)
        self.setup.ARCHIVE_BYTES = self.archive.stat().st_size
        self.setup.ARCHIVE_SHA256 = self.setup.digest(self.archive)
        self.setup.DICTIONARY_BYTES = len(self.files["system_core.dic"])
        self.setup.DICTIONARY_SHA256 = hashlib.sha256(self.files["system_core.dic"]).hexdigest()
        self.setup.FILES = {
            name: (len(data), hashlib.sha256(data).hexdigest())
            for name, data in self.files.items()
        }
        self.setup.URL = self.archive.as_uri()

    def prepare(self, archive):
        with contextlib.redirect_stdout(io.StringIO()) as output:
            self.setup.prepare(archive)
        return output.getvalue()

    def installed_files(self):
        bundle = (self.destination / "current").resolve()
        return {name: (bundle / name).read_bytes() for name in self.files}

    def test_cli_failure_names_the_dictionary_setup_and_preserves_the_error(self):
        with mock.patch("sys.argv", ["setup", "--archive", str(self.root / "missing.zip")]):
            with contextlib.redirect_stderr(io.StringIO()) as output:
                with self.assertRaises(SystemExit) as failure:
                    self.setup.main()
        self.assertEqual(failure.exception.code, 1)
        self.assertTrue(output.getvalue().startswith("Sudachi dictionary setup failed: "))
        self.assertIn("missing.zip", output.getvalue())

    def test_damaged_notice_is_repaired_instead_of_reported_ready(self):
        self.prepare(self.archive)
        for name in ("LEGAL", "LICENSE-2.0.txt"):
            with self.subTest(name=name):
                notice = self.destination / "current" / name
                notice.write_bytes(b"x" * len(self.files[name]))
                self.prepare(None)
                self.assertEqual(self.installed_files(), self.files)

    def test_verified_bundle_is_reused_without_reading_an_archive(self):
        self.prepare(self.archive)
        published = (self.destination / "current").resolve()
        self.archive.unlink()
        self.assertIn("already ready", self.prepare(None))
        self.assertEqual((self.destination / "current").resolve(), published)
        self.assertEqual(self.installed_files(), self.files)

    def test_missing_notice_is_repaired(self):
        self.prepare(self.archive)
        (self.destination / "current" / "LICENSE-2.0.txt").unlink()
        self.prepare(None)
        self.assertEqual(self.installed_files(), self.files)

    def test_invalid_archive_preserves_current_and_legacy_files(self):
        self.prepare(self.archive)
        legacy = self.destination / "LEGAL"
        legacy.write_bytes(b"legacy notice")
        self.archive.write_bytes(b"invalid archive")
        with self.assertRaisesRegex(ValueError, "Archive does not match"):
            self.prepare(self.archive)
        self.assertEqual(self.installed_files(), self.files)
        self.assertEqual(legacy.read_bytes(), b"legacy notice")

    def test_real_rename_failure_preserves_a_directory_at_current(self):
        current = self.destination / "current"
        current.mkdir(parents=True)
        for name, data in self.files.items():
            (current / name).write_bytes(data)
        with self.assertRaises(OSError):
            self.prepare(self.archive)
        self.assertFalse(current.is_symlink())
        self.assertEqual(self.installed_files(), self.files)

    def test_failed_publication_preserves_the_complete_previous_bundle(self):
        self.prepare(self.archive)
        previous = self.installed_files()
        previous_bundle = (self.destination / "current").resolve()
        self.files = {name: b"replacement " + data for name, data in self.files.items()}
        self.configure_archive()
        replace = Path.replace

        def fail_publication(source, destination):
            if destination == self.destination / "current":
                raise OSError("injected publication failure")
            return replace(source, destination)

        with mock.patch.object(Path, "replace", fail_publication):
            with self.assertRaisesRegex(OSError, "injected publication failure"):
                self.prepare(self.archive)
        self.assertEqual(self.installed_files(), previous)
        self.prepare(self.archive)
        self.assertEqual(self.installed_files(), self.files)
        self.assertEqual(
            {name: (previous_bundle / name).read_bytes() for name in previous}, previous
        )

    def test_unexpected_extracted_notice_preserves_the_previous_bundle(self):
        self.prepare(self.archive)
        previous = self.installed_files()
        notice_pin = self.setup.FILES["LEGAL"]
        self.files["LEGAL"] = b"unexpected notice"
        self.configure_archive()
        self.setup.FILES["LEGAL"] = notice_pin
        with self.assertRaisesRegex(ValueError, "bundle failed verification"):
            self.prepare(self.archive)
        self.assertEqual(self.installed_files(), previous)

    def test_sync_failure_before_publication_preserves_previous_bundle(self):
        self.prepare(self.archive)
        previous = self.installed_files()
        self.files = {name: b"replacement " + data for name, data in self.files.items()}
        self.configure_archive()
        with mock.patch.object(os, "fsync", side_effect=OSError("injected file sync failure")):
            with self.assertRaisesRegex(OSError, "injected file sync failure"):
                self.prepare(self.archive)
        self.assertEqual(self.installed_files(), previous)

    def test_sync_failure_after_publication_reports_uncertain_durability(self):
        self.prepare(self.archive)
        previous_bundle = (self.destination / "current").resolve()
        self.files = {name: b"replacement " + data for name, data in self.files.items()}
        self.configure_archive()
        fsync = os.fsync

        def fail_after_publication(descriptor):
            if (self.destination / "current").resolve() != previous_bundle:
                raise OSError("injected directory sync failure")
            fsync(descriptor)

        with mock.patch.object(os, "fsync", fail_after_publication):
            with self.assertRaisesRegex(OSError, "durability is uncertain") as failure:
                self.prepare(self.archive)
        self.assertIsInstance(failure.exception, self.setup.DurabilityUncertain)
        self.assertIsInstance(failure.exception.__cause__, OSError)
        self.assertEqual(self.installed_files(), self.files)


if __name__ == "__main__":
    unittest.main()
