"""Exercise the real provider-placement scan, including its path/fixture scope."""

import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


RULE = Path(__file__).with_name("provider_placement.yml")


class ProviderPlacement(unittest.TestCase):
    def scan(self, path, source, rejected=False):
        with tempfile.TemporaryDirectory(prefix="yomibu-provider-check-") as directory:
            root = Path(directory)
            shutil.copyfile(RULE, root / RULE.name)
            target = root / path
            target.parent.mkdir(parents=True)
            target.write_text(source, encoding="utf-8")
            result = subprocess.run(
                ["ast-grep", "scan", "--rule", RULE.name, "--json=compact", "."],
                cwd=root, capture_output=True, text=True, timeout=30,
            )
        self.assertEqual(result.returncode, int(rejected), result.stdout + result.stderr)
        findings = json.loads(result.stdout)
        self.assertEqual(bool(findings), rejected)
        for finding in findings:
            self.assertEqual(finding["ruleId"], "provider-placement")
            self.assertEqual(finding["severity"], "error")
            self.assertEqual(Path(finding["file"]), Path(path))

    def test_rejects_provider_identifiers_in_core_and_app(self):
        for crate in ("yomibu-core", "yomibu-app"):
            for source in (
                "pub trait WaniKaniSubjectQuery {}",
                "pub mod wani_kani {}",
                "pub struct OpenAiGenerator;",
                "pub enum AnthropicModel {}",
                "pub fn sudachi_assessment() {}",
                "pub struct JevEntry;",
            ):
                with self.subTest(crate=crate, source=source):
                    self.scan(f"crates/{crate}/src/lib.rs", source, rejected=True)

    def test_rejects_hardcoded_provider_identities(self):
        for literal in ('"wanikani"', 'r#"openai"#', '"Anthropic"'):
            with self.subTest(literal=literal):
                self.scan("crates/yomibu-core/src/domain/mod.rs",
                          f"pub const DEFAULT_SOURCE: &str = {literal};", rejected=True)

    def test_allows_comments_and_opaque_input(self):
        self.scan("crates/yomibu-core/src/lib.rs", '''
            // WaniKani is an integration, as are OpenAI and Anthropic.
            /// Sudachi and JEV are examples, not inner types.
            pub trait SubjectQuery {}
            pub fn accepts_source_identity(source: &str) -> &str { source }
            pub const PAYLOAD: &str = r#"{"provider":"wanikani"}"#;
        ''')

    def test_allows_integrations_hosts_and_integration_tests(self):
        for path in ("crates/yomibu-ext/src/wanikani.rs", "crates/yomibu-cli/src/main.rs",
                     "crates/yomibu-core/tests/import.rs", "crates/yomibu-app/tests/import.rs"):
            with self.subTest(path=path):
                self.scan(path, 'pub struct WaniKaniClient; const PROVIDER: &str = "wanikani";')

    def test_allows_inline_test_fixtures(self):
        for separator in ("#[allow(dead_code)]", "// fixture", "/* fixture */"):
            with self.subTest(separator=separator):
                self.scan("crates/yomibu-core/src/lib.rs", f'''
                    #[cfg(test)]
                    {separator}
                    mod tests {{ pub struct WaniKaniFixture; const SOURCE: &str = "wanikani"; }}
                ''')

    def test_fixture_exemption_does_not_extend_to_production(self):
        self.scan("crates/yomibu-core/src/lib.rs", '''
            #[cfg(test)] mod tests {}
            pub trait WaniKaniSubjectQuery {}
        ''', rejected=True)
