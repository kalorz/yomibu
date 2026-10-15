"""Exercise architecture checks against real, isolated Cargo workspaces."""

import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


CHECKER = Path(__file__).with_name("check_architecture.py")
CRATES = ("yomibu-core", "yomibu-app", "yomibu-ext", "yomibu-cli")


class CrateDependencies(unittest.TestCase):
    def check_workspace(self, declarations=None, members=CRATES):
        declarations = declarations or {}
        with tempfile.TemporaryDirectory(prefix="yomibu-architecture-") as directory:
            root = Path(directory)
            (root / "Cargo.toml").write_text(
                "[workspace]\nresolver = '3'\nmembers = "
                + json.dumps(list(members))
                + "\n",
                encoding="utf-8",
            )
            for name in members:
                package = root / name
                (package / "src").mkdir(parents=True)
                (package / "src/lib.rs").write_text("", encoding="utf-8")
                (package / "Cargo.toml").write_text(
                    f"[package]\nname = '{name}'\nversion = '0.0.0'\n"
                    "edition = '2024'\n" + declarations.get(name, ""),
                    encoding="utf-8",
                )
            locked = subprocess.run(
                ["cargo", "generate-lockfile", "--offline", "--manifest-path", str(root / "Cargo.toml")],
                cwd=CHECKER.parent.parent, text=True, capture_output=True, timeout=30,
            )
            self.assertEqual(locked.returncode, 0, locked.stderr)
            return subprocess.run(
                [sys.executable, str(CHECKER), "--manifest-path", str(root / "Cargo.toml")],
                text=True, capture_output=True, timeout=30,
            )

    def assert_rejected(self, result, message):
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn(message, result.stderr)

    def test_accepts_the_agreed_graph_and_reviewed_app_integration_dependency(self):
        result = self.check_workspace({
            "yomibu-app": "[dependencies]\nyomibu-core = { path = '../yomibu-core' }\n"
                          "[dev-dependencies]\nyomibu-ext = { path = '../yomibu-ext' }\n",
            "yomibu-ext": "[dependencies]\nyomibu-core = { path = '../yomibu-core' }\n",
            "yomibu-cli": "[dependencies]\nyomibu-core = { path = '../yomibu-core' }\n"
                          "yomibu-app = { path = '../yomibu-app' }\n"
                          "yomibu-ext = { path = '../yomibu-ext' }\n",
        })
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Crate dependency checks passed", result.stdout)

    def test_rejects_app_to_ext_production_dependency(self):
        result = self.check_workspace({
            "yomibu-app": "[dependencies]\nyomibu-ext = { path = '../yomibu-ext' }\n",
        })
        self.assert_rejected(result, "yomibu-app -> yomibu-ext")

    def test_rejects_renamed_workspace_dependency(self):
        result = self.check_workspace({
            "yomibu-app": "[dependencies]\nprovider = { package = 'yomibu-ext', path = '../yomibu-ext' }\n",
        })
        self.assert_rejected(result, "yomibu-app -> yomibu-ext")

    def test_checks_optional_dependencies(self):
        result = self.check_workspace({
            "yomibu-ext": "[dependencies]\nyomibu-app = { path = '../yomibu-app', optional = true }\n",
        })
        self.assert_rejected(result, "yomibu-ext -> yomibu-app")

    def test_checks_platform_specific_dependencies(self):
        result = self.check_workspace({
            "yomibu-core": "[target.'cfg(windows)'.dependencies]\n"
                           "yomibu-app = { path = '../yomibu-app' }\n",
        })
        self.assert_rejected(result, "yomibu-core -> yomibu-app")
        self.assertIn("cfg(windows)", result.stderr)

    def test_checks_build_dependencies(self):
        result = self.check_workspace({
            "yomibu-core": "[build-dependencies]\nyomibu-cli = { path = '../yomibu-cli' }\n",
        })
        self.assert_rejected(result, "yomibu-core -> yomibu-cli")
        self.assertIn("build", result.stderr)

    def test_does_not_blanket_exempt_dev_dependencies(self):
        result = self.check_workspace({
            "yomibu-core": "[dev-dependencies]\nyomibu-ext = { path = '../yomibu-ext' }\n",
        })
        self.assert_rejected(result, "yomibu-core -> yomibu-ext")

    def test_accepts_future_driving_crates_in_the_cli_layer(self):
        result = self.check_workspace({
            name: "[dependencies]\nyomibu-app = { path = '../yomibu-app' }\n"
                  "yomibu-ext = { path = '../yomibu-ext' }\n"
            for name in ("yomibu-web", "yomibu-mcp")
        }, members=CRATES + ("yomibu-web", "yomibu-mcp"))
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_rejects_dependencies_between_driving_crates(self):
        result = self.check_workspace({
            "yomibu-web": "[dependencies]\nyomibu-cli = { path = '../yomibu-cli' }\n",
        }, members=CRATES + ("yomibu-web",))
        self.assert_rejected(result, "yomibu-web -> yomibu-cli")

    def test_requires_explicit_classification_of_new_workspace_crates(self):
        result = self.check_workspace(members=CRATES + ("new-component",))
        self.assert_rejected(result, "Unclassified workspace crate: new-component")

    def test_rejects_a_missing_retained_crate(self):
        result = self.check_workspace(members=CRATES[1:])
        self.assert_rejected(result, "Missing retained workspace crate: yomibu-core")

    def test_metadata_failure_is_a_check_error_not_success(self):
        with tempfile.TemporaryDirectory(prefix="yomibu-architecture-invalid-") as directory:
            result = subprocess.run(
                [sys.executable, str(CHECKER), "--manifest-path", str(Path(directory) / "Cargo.toml")],
                text=True, capture_output=True, timeout=30,
            )
        self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
        self.assertIn("Cargo metadata failed", result.stderr)


if __name__ == "__main__":
    unittest.main()
