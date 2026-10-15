"""Test Yomibu's rules and frozen exceptions, not Bylaw's Rust analysis."""

import copy
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

from check_domain import check_baseline


class DomainBaseline(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory(prefix="yomibu-baseline-")
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name).resolve()
        self.source = self.root / "domain.rs"
        self.source.write_text("use crate::ports::Selector;\n", encoding="utf-8")
        self.finding = {
            "message": "domain depends on ports",
            "target": {"kind": "module", "id": "yomibu_core::ports"},
            "evidence": [{"description": "crate::ports::Selector",
                          "span": {"path": str(self.source)}}],
        }
        self.report = {"version": 1, "report": {
            "analysis_diagnostics": [],
            "rule_results": [{"rule_id": "domain-boundary", "severity": "error",
                              "violations": [self.finding]},
                             {"rule_id": "inner-libraries", "severity": "error",
                              "violations": []}],
        }}
        self.baseline = {"domain.rs": {
            "sha256": hashlib.sha256(self.source.read_bytes()).hexdigest(),
            "violations": [self.finding["message"]],
        }}

    def check(self, baseline=None):
        return check_baseline(self.root, self.report,
                              self.baseline if baseline is None else baseline)

    def test_accepts_only_the_exact_existing_source_and_findings(self):
        self.assertEqual(self.check(), [])
        self.source.write_text(self.source.read_text() + "// changed\n", encoding="utf-8")
        self.assertTrue(self.check())

    def test_rejects_a_new_violation_even_in_an_unchanged_file(self):
        self.finding["message"] = "domain depends on another port"
        self.assertTrue(self.check())

    def test_rejects_unbaselined_source_in_shared_evidence(self):
        self.finding["evidence"].append({"description": "Selector",
                                      "span": {"path": str(self.root / "new.rs")}})
        self.assertTrue(self.check())

    def test_allows_namespace_prefix_but_rejects_crate_root_items(self):
        self.finding["target"]["id"] = "package::yomibu_core"
        for namespace in ("crate", "super", "super::super", "self::super"):
            self.finding["evidence"][0]["description"] = namespace
            self.assertEqual(self.check({}), [])
        self.finding["evidence"].append({"description": "crate::outside",
                                      "span": {"path": str(self.source)}})
        self.assertTrue(self.check({}))

    def test_rejects_new_violations_without_a_baseline(self):
        self.assertTrue(self.check({}))

    def test_library_violations_never_use_the_domain_baseline(self):
        library = self.report["report"]["rule_results"][1]
        for evidence in (self.finding["evidence"], [{"span": None}]):
            library["violations"] = [{**self.finding, "evidence": evidence}]
            self.assertEqual(self.check(), [self.finding["message"]])

    def test_requires_removal_of_fixed_or_deleted_exceptions(self):
        self.report["report"]["rule_results"][0]["violations"] = []
        self.assertTrue(self.check())
        self.source.unlink()
        self.assertTrue(self.check())
        self.assertEqual(self.check({}), [])

    def test_rejects_analysis_diagnostics_even_if_downgraded_to_warnings(self):
        self.report["report"]["analysis_diagnostics"] = [{"severity": "warning"}]
        with self.assertRaises(ValueError):
            self.check()

    def test_rejects_missing_or_weakened_rule_and_unknown_report_format(self):
        original = copy.deepcopy(self.report)
        for change in ("missing", "warning", "missing-library", "library-warning",
                       "duplicate", "version", "evidence"):
            with self.subTest(change=change):
                self.report = copy.deepcopy(original)
                rules = self.report["report"]["rule_results"]
                if change == "missing":
                    rules.clear()
                elif change == "warning":
                    rules[0]["severity"] = "warning"
                elif change == "missing-library":
                    rules.pop()
                elif change == "library-warning":
                    rules[1]["severity"] = "warning"
                elif change == "duplicate":
                    rules.append(copy.deepcopy(rules[0]))
                elif change == "version":
                    self.report["version"] = 2
                else:
                    rules[0]["violations"][0]["evidence"] = []
                with self.assertRaises(ValueError):
                    self.check()


class DomainRule(unittest.TestCase):
    def test_configured_rule_accepts_domain_data_and_rejects_boundary_crossings(self):
        project = Path(__file__).resolve().parents[1]
        cases = [
            ("pub struct V; pub mod nested { use crate::{domain::V}; pub fn f(_: V) {} }", False),
            ("pub struct V; use super::{domain::V as W}; pub fn f(_: W) {}", False),
            ("pub struct V; pub mod nested { use super::super::{domain::V}; pub fn f(_: V) {} }", False),
            ("macro_rules! bad { () => { pub fn f(_: &dyn crate::ports::Selector) {} } } bad!();", True),
            ("pub fn f() { crate::outside(); }", True),
        ]
        for source, rejected in cases:
            with self.subTest(source=source), tempfile.TemporaryDirectory() as directory:
                root = Path(directory).resolve()
                (root / "src").mkdir()
                (root / "Cargo.toml").write_text(
                    '[package]\nname="yomibu-core"\nversion="0.0.0"\nedition="2024"\n[workspace]\n')
                (root / "rust-toolchain.toml").write_bytes((project / "rust-toolchain.toml").read_bytes())
                (root / "src/lib.rs").write_text(
                    'pub mod domain; pub mod ports { pub trait Selector {} } pub fn outside() {}')
                (root / "src/domain.rs").write_text(source)
                result = subprocess.run(
                    ["cargo", "bylaw", "check", "--config", str(project / "bylaw.toml"),
                     "--manifest-path", str(root / "Cargo.toml"),
                     "--package", "yomibu-core", "--format", "json"],
                    cwd=project, capture_output=True, text=True, timeout=60,
                )
                self.assertIn(result.returncode, (0, 1), result.stderr or result.stdout)
                self.assertEqual(bool(check_baseline(root, json.loads(result.stdout), {})), rejected)


class LibraryRule(unittest.TestCase):
    def test_core_and_app_reject_provider_dependencies_but_allow_test_dependencies(self):
        project = Path(__file__).resolve().parents[1]
        for package in ("yomibu-core", "yomibu-app"):
            for kind in ("dependencies", "dev-dependencies"):
                with self.subTest(package=package, kind=kind), tempfile.TemporaryDirectory() as directory:
                    root = Path(directory).resolve()
                    for name in ("inner", "provider"):
                        (root / name / "src").mkdir(parents=True)
                    (root / "provider/Cargo.toml").write_text(
                        '[package]\nname="provider-fixture"\nversion="0.0.0"\n[workspace]\n')
                    (root / "provider/src/lib.rs").write_text("pub struct Client;")
                    (root / "inner/Cargo.toml").write_text(
                        f'[package]\nname="{package}"\nversion="0.0.0"\nedition="2024"\n'
                        f'[workspace]\n[{kind}]\n'
                        'renamed={package="provider-fixture",path="../provider"}\n')
                    (root / "inner/src/lib.rs").write_text("pub mod domain { pub struct Value; }")
                    (root / "rust-toolchain.toml").write_bytes((project / "rust-toolchain.toml").read_bytes())
                    result = subprocess.run(
                        ["cargo", "bylaw", "check", "--config", str(project / "bylaw.toml"),
                         "--manifest-path", str(root / "inner/Cargo.toml"),
                         "--package", package, "--format", "json"],
                        cwd=project, capture_output=True, text=True, timeout=60,
                    )
                    self.assertIn(result.returncode, (0, 1), result.stderr or result.stdout)
                    errors = check_baseline(root, json.loads(result.stdout), {})
                    self.assertEqual(bool(errors), kind == "dependencies", errors)
