"""Test Yomibu's dependency rules and Bylaw namespace correction."""

import copy
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

from check_domain import check_report


class DomainReport(unittest.TestCase):
    def setUp(self):
        self.finding = {
            "message": "domain depends on ports",
            "target": {"kind": "module", "id": "yomibu_core::ports"},
            "evidence": [{"description": "crate::ports::Selector",
                          "span": {"path": "domain.rs"}}],
        }
        self.report = {"version": 1, "report": {
            "analysis_diagnostics": [],
            "rule_results": [{"rule_id": "domain-boundary", "severity": "error",
                              "violations": [self.finding]},
                             {"rule_id": "inner-libraries", "severity": "error",
                              "violations": []}],
        }}

    def test_allows_namespace_prefix_but_rejects_crate_root_items(self):
        self.finding["target"]["id"] = "package::yomibu_core"
        for namespace in ("crate", "super", "super::super", "self::super"):
            self.finding["evidence"][0]["description"] = namespace
            self.assertEqual(check_report(self.report), [])
        self.finding["evidence"].append({"description": "crate::outside",
                                      "span": {"path": "domain.rs"}})
        self.assertEqual(check_report(self.report), [self.finding["message"]])

    def test_rejects_domain_violations_and_accepts_clean_reports(self):
        self.assertEqual(check_report(self.report), [self.finding["message"]])
        self.report["report"]["rule_results"][0]["violations"] = []
        self.assertEqual(check_report(self.report), [])

    def test_rejects_library_violations_including_unlocated_declarations(self):
        self.report["report"]["rule_results"][0]["violations"] = []
        library = self.report["report"]["rule_results"][1]
        for evidence in (self.finding["evidence"], [{"span": None}]):
            library["violations"] = [{**self.finding, "evidence": evidence}]
            self.assertEqual(check_report(self.report), [self.finding["message"]])

    def test_rejects_analysis_diagnostics_even_if_downgraded_to_warnings(self):
        self.report["report"]["analysis_diagnostics"] = [{"severity": "warning"}]
        with self.assertRaises(ValueError):
            check_report(self.report)

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
                    check_report(self.report)


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
                self.assertEqual(bool(check_report(json.loads(result.stdout))), rejected)


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
                    errors = check_report(json.loads(result.stdout))
                    self.assertEqual(bool(errors), kind == "dependencies", errors)
