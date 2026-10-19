"""Verify scoped domain/application restrictions with isolated Rust examples."""

import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import unittest


def extract_lint_header(source):
    # These policy files use one forbid attribute; exclude docs and declarations.
    attribute = re.search(r"(?m)^#!\[forbid\([^]]*\)\]", source)
    if attribute is None:
        raise ValueError("Missing or incomplete forbid attribute")
    return attribute.group() + "\n"


class LintHeader(unittest.TestCase):
    def test_extracts_attribute_after_docs_and_preserves_newline(self):
        attribute = '#![forbid(\n    clippy::disallowed_methods,\n\n    clippy::print_stdout,\n)]'
        source = '//! Domain values.\n\n' + attribute + '\n\npub struct Value;'
        self.assertEqual(extract_lint_header(source), attribute + '\n')

    def test_rejects_missing_or_incomplete_attribute(self):
        for source in ('', '//! Domain values.\n', '#![allow(clippy::print_stdout)]',
                       '#![forbid(clippy::print_stdout'):
            with self.subTest(source=source), self.assertRaises(ValueError):
                extract_lint_header(source)


class InnerClippy(unittest.TestCase):
    def test_inner_layers_reject_external_access_but_allow_pure_utilities(self):
        cases = [
            ('pub fn read() -> std::io::Result<Vec<u8>> { std::fs::read("fixture") }', "disallowed_methods"),
            ('pub fn write() -> std::io::Result<()> { std::fs::write("fixture", b"data") }', "disallowed_methods"),
            ('use std::fs::remove_file as remove; pub fn delete() -> std::io::Result<()> { let f = remove::<&str>; f("fixture") }', "disallowed_methods"),
            ('pub fn setting() -> Option<std::ffi::OsString> { std::env::var_os("INPUT") }', "disallowed_methods"),
            ('pub fn directory() -> std::io::Result<std::path::PathBuf> { std::env::current_dir() }', "disallowed_methods"),
            ('pub fn arguments() -> std::env::ArgsOs { std::env::args_os() }', "disallowed_methods"),
            ('pub fn exists(p: &std::path::Path) -> bool { p.exists() }', "disallowed_methods"),
            ('pub fn input() -> std::io::Stdin { std::io::stdin() }', "disallowed_methods"),
            ('pub fn open() -> std::fs::OpenOptions { std::fs::OpenOptions::new() }', "disallowed_types"),
            ('pub fn command() -> std::process::Command { std::process::Command::new("fixture") }', "disallowed_types"),
            ('pub fn stop() -> ! { std::process::exit(1) }', "disallowed_methods"),
            ('pub fn stop() -> ! { std::process::abort() }', "disallowed_methods"),
            ('pub fn network(_: std::net::UdpSocket) {}', "disallowed_types"),
            ('use std::net::ToSocketAddrs; pub fn lookup() -> std::io::Result<impl Iterator<Item=std::net::SocketAddr>> { "example.com:80".to_socket_addrs() }', "disallowed_methods"),
            ('pub fn pipe() -> std::io::Result<(std::io::PipeReader, std::io::PipeWriter)> { std::io::pipe() }', "disallowed_methods"),
            ('#[allow(deprecated)] pub fn link() -> std::io::Result<()> { std::fs::soft_link("a", "b") }', "disallowed_methods"),
            ('pub const CONTENT: &[u8] = include_bytes!("../Cargo.toml");', "disallowed_macros"),
            ('pub fn debug() { dbg!(1); }', "disallowed_macros"),
            ('unsafe extern "C" { pub safe fn abort() -> !; }', 'usage of an `unsafe extern` block'),
            ('pub fn extension(p: &std::path::Path) -> Option<&std::ffi::OsStr> { p.extension() }', None),
            ('pub fn parts(value: &std::ffi::OsStr) -> usize { std::env::split_paths(value).count() }', None),
            ('pub fn buffer(bytes: Vec<u8>) -> std::io::Cursor<Vec<u8>> { std::io::Cursor::new(bytes) }', None),
        ]
        if os.name == "posix":
            cases += [
                ('pub fn link() -> std::io::Result<()> { std::os::unix::fs::symlink("a", "b") }', "disallowed_methods"),
                ('pub fn network(_: std::os::unix::net::UnixStream) {}', "disallowed_types"),
                ('pub fn network(_: std::os::unix::net::UnixListener) {}', "disallowed_types"),
                ('pub fn network(_: std::os::unix::net::UnixDatagram) {}', "disallowed_types"),
            ]
        for crate, policy in (("yomibu-core", "src/domain/mod.rs"), ("yomibu-app", "src/lib.rs")):
            with self.subTest(crate=crate):
                self.check_cases(crate, policy, cases)

    def test_domain_restrictions_reject_uuid_generation_when_features_are_enabled(self):
        project = Path(__file__).resolve().parents[1]
        metadata = subprocess.run(
            ["cargo", "metadata", "--locked", "--format-version", "1"],
            cwd=project, capture_output=True, text=True, check=True, timeout=60,
        )
        uuid = next(package for package in json.loads(metadata.stdout)["packages"]
                    if package["name"] == "uuid")
        self.check_cases("yomibu-core", "src/domain/mod.rs", [
            ("pub fn id() -> uuid::Uuid { uuid::Uuid::from_u128(7) }", None),
            ("pub fn id() -> uuid::Uuid { uuid::Uuid::new_v4() }", "disallowed_methods"),
            ("use uuid::Uuid as Id; pub fn id() -> Id { let generate = Id::new_v4; generate() }", "disallowed_methods"),
            ("pub fn id() -> uuid::Uuid { uuid::Uuid::now_v7() }", "disallowed_methods"),
            ("pub fn context(_: uuid::ContextV7) {}", "disallowed_types"),
        ], dependencies=(
            f'uuid = {{ version = "={uuid["version"]}", default-features = false, '
            'features = ["std", "v4", "v7"] }\n'
        ))

    def test_chrono_clock_features_preserve_domain_purity_and_application_timing(self):
        project = Path(__file__).resolve().parents[1]
        metadata = subprocess.run(
            ["cargo", "metadata", "--locked", "--format-version", "1"],
            cwd=project, capture_output=True, text=True, check=True, timeout=60,
        )
        chrono = next(package for package in json.loads(metadata.stdout)["packages"]
                      if package["name"] == "chrono")
        dependency = (f'chrono = {{ version = "={chrono["version"]}", '
                      'default-features = false, features = ["std", "clock"] }\n')
        self.check_cases("yomibu-core", "src/domain/mod.rs", [
            ("pub fn supplied(seconds: i64) -> Option<chrono::DateTime<chrono::Utc>> { chrono::DateTime::from_timestamp(seconds, 0) }", None),
            ("pub fn now() -> chrono::DateTime<chrono::Utc> { chrono::Utc::now() }", "disallowed_methods"),
            ("use chrono::Utc as Clock; pub fn now() -> chrono::DateTime<Clock> { let clock = Clock::now; clock() }", "disallowed_methods"),
            ("pub fn now() -> chrono::DateTime<chrono::Local> { chrono::Local::now() }", "disallowed_methods"),
            ("use chrono::Local as Clock; pub fn now() -> chrono::DateTime<Clock> { let clock = Clock::now; clock() }", "disallowed_methods"),
            ("#[allow(deprecated)] pub fn today() -> chrono::Date<chrono::Utc> { chrono::Utc::today() }", "disallowed_methods"),
            ("#[allow(deprecated)] pub fn today() -> chrono::Date<chrono::Local> { chrono::Local::today() }", "disallowed_methods"),
            ("pub fn local(_: chrono::DateTime<chrono::Local>) {}", "disallowed_types"),
        ], dependencies=dependency)
        self.check_cases("yomibu-app", "src/lib.rs", [
            ("pub fn now() -> chrono::DateTime<chrono::Utc> { chrono::Utc::now() }", None),
        ], dependencies=dependency)

    def test_domain_restrictions_preserve_shell_and_fixture_operations(self):
        self.check_cases("yomibu-core", "src/domain/mod.rs", [
            ("pub fn duration(d: std::time::Duration) -> u128 { d.as_millis() }", None),
            ("use std::time::Instant as Clock; pub fn now() -> Clock { let f = Clock::now; f() }", "disallowed_methods"),
            ("pub fn elapsed() -> Result<std::time::Duration, std::time::SystemTimeError> { std::time::UNIX_EPOCH.elapsed() }", "disallowed_methods"),
            ("pub fn file(_: std::fs::File) {}", "disallowed_types"),
            ('pub const SETTING: Option<&str> = option_env!("YOMIBU_LINT_FIXTURE");', "disallowed_macros"),
            ('pub fn report() { println!("output"); }', "print_stdout"),
            ("#[allow(clippy::disallowed_methods)] pub fn now() -> std::time::Instant { std::time::Instant::now() }", "E0453"),
        ])

    def test_app_io_restrictions_preserve_timing_and_fixture_operations(self):
        self.check_cases("yomibu-app", "src/lib.rs", [
            ("pub fn timing() -> std::time::Duration { std::time::Instant::now().elapsed() }", None),
            ("pub fn elapsed() -> Result<std::time::Duration, std::time::SystemTimeError> { std::time::UNIX_EPOCH.elapsed() }", None),
            ('pub fn read() -> std::io::Result<String> { std::fs::read_to_string("input.json") }', "disallowed_methods"),
            ('use std::env::var as setting; pub fn read() -> Result<String, std::env::VarError> { setting("INPUT") }', "disallowed_methods"),
            ("pub fn network(_: std::net::TcpStream) {}", "disallowed_types"),
            ('pub const SETTING: Option<&str> = option_env!("YOMIBU_LINT_FIXTURE");', "disallowed_macros"),
            ('pub fn report() { println!("output"); }', "print_stdout"),
        ])

    def check_cases(self, crate_name, policy_path, cases, dependencies=""):
        project = Path(__file__).resolve().parents[1]
        crate = project / "crates" / crate_name
        lint_header = extract_lint_header((crate / policy_path).read_text())
        _, lint_section, lint_settings = (crate / "Cargo.toml").read_text().partition("[lints.rust]")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "src").mkdir()
            (root / "tests").mkdir()
            (root / "Cargo.toml").write_text(
                '[package]\nname="inner-lint-fixture"\nversion="0.0.0"\nedition="2024"\n'
                '[workspace]\n[dependencies]\n' + dependencies + lint_section + lint_settings)
            (root / "rust-toolchain.toml").write_bytes((project / "rust-toolchain.toml").read_bytes())
            (root / "clippy.toml").write_bytes((crate / "clippy.toml").read_bytes())
            (root / "tests/fixture.rs").write_text(
                '#[test] fn fixture_io() {'
                'let _ = include_str!("../Cargo.toml");'
                'let _ = include_bytes!("../Cargo.toml");'
                'let _ = std::fs::read("fixture");'
                'let _ = std::env::var_os("FIXTURE"); }')
            if dependencies:
                # Keep shared dependencies on the workspace's reviewed versions.
                (root / "Cargo.lock").write_bytes((project / "Cargo.lock").read_bytes())
                # Generation features may need crates absent from the production graph.
                subprocess.run(
                    ["cargo", "fetch"], cwd=root, capture_output=True, text=True,
                    check=True, timeout=60,
                )
            for source, diagnostic in cases:
                with self.subTest(source=source, diagnostic=diagnostic):
                    (root / "src/lib.rs").write_text(
                        'pub mod checked {\n' + lint_header + source + '\n}\n'
                        'pub fn shell_clock() -> std::time::Instant { std::time::Instant::now() }'
                        'pub fn shell_io() -> std::io::Result<Vec<u8>> { std::fs::read("fixture") }')
                    result = subprocess.run(
                        ["cargo", "clippy", "--offline", "--all-targets", "--", "-D", "warnings"],
                        cwd=root, capture_output=True, text=True, timeout=60,
                    )
                    if diagnostic is None:
                        self.assertEqual(result.returncode, 0, result.stderr)
                    else:
                        self.assertNotEqual(result.returncode, 0, result.stderr)
                        self.assertIn(diagnostic, result.stderr)
