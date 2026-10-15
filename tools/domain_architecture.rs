//! Source-boundary guard, not a proof of functional purity. See ARCHITECTURE.md.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use sha2::{Digest, Sha256};
use syn::ext::IdentExt;
use syn::visit::{self, Visit};

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Baseline {
    sha256: String,
    violations: Vec<String>,
}

struct Inspector {
    module: Vec<String>,
    findings: BTreeSet<String>,
}

impl Inspector {
    fn path(&mut self, segments: &[String]) {
        let segments: Vec<String> = segments
            .iter()
            .map(|s| s.strip_prefix("r#").unwrap_or(s).to_owned())
            .collect();
        let Some(first) = segments.first() else {
            return;
        };
        if matches!(first.as_str(), "std" | "core")
            && segments.last().map(String::as_str) == Some("include")
        {
            self.findings
                .insert("domain imports include!; source must stay in the scanned tree".into());
        }
        let mut resolved = match first.as_str() {
            "crate" | "yomibu_core" => Vec::new(),
            "self" | "super" => self.module.clone(),
            // Unqualified names and external libraries require semantic review.
            _ => return,
        };
        let start = usize::from(matches!(first.as_str(), "crate" | "yomibu_core"));
        for segment in &segments[start..] {
            match segment.as_str() {
                "self" => {}
                "super" => {
                    resolved.pop();
                }
                _ => resolved.push(segment.clone()),
            }
        }
        if resolved.first().map(String::as_str) != Some("domain") {
            self.findings
                .insert(format!("domain depends on crate::{}", resolved.join("::")));
        }
    }

    fn import(&mut self, tree: &syn::UseTree, prefix: &[String]) {
        let mut path = prefix.to_vec();
        match tree {
            syn::UseTree::Path(node) => {
                path.push(node.ident.to_string());
                self.import(&node.tree, &path);
            }
            syn::UseTree::Group(node) => {
                for item in &node.items {
                    self.import(item, prefix);
                }
            }
            syn::UseTree::Name(node) => {
                path.push(node.ident.to_string());
                self.path(&path);
            }
            syn::UseTree::Rename(node) => {
                path.push(node.ident.to_string());
                self.path(&path);
            }
            syn::UseTree::Glob(_) => self.path(&path),
        }
    }
}

impl<'ast> Visit<'ast> for Inspector {
    fn visit_path(&mut self, path: &'ast syn::Path) {
        self.path(
            &path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect::<Vec<_>>(),
        );
        visit::visit_path(self, path);
    }

    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        self.import(&item.tree, &[]);
    }

    fn visit_item_extern_crate(&mut self, item: &'ast syn::ItemExternCrate) {
        if item.ident.unraw() == "self" || item.ident.unraw() == "yomibu_core" {
            self.findings
                .insert("domain aliases the core crate root".into());
        }
    }

    fn visit_visibility(&mut self, _visibility: &'ast syn::Visibility) {
        // pub(crate) and pub(in ...) restrict visibility, not dependencies.
    }

    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        if item.attrs.iter().any(|a| redirects_module(&a.meta)) {
            self.findings
                .insert("domain module uses #[path]; source must stay in the scanned tree".into());
        }
        if let Some((_, items)) = &item.content {
            self.module.push(item.ident.unraw().to_string());
            for nested in items {
                self.visit_item(nested);
            }
            self.module.pop();
        }
    }

    fn visit_macro(&mut self, item: &'ast syn::Macro) {
        if item
            .path
            .segments
            .last()
            .is_some_and(|s| s.ident.unraw() == "include")
        {
            self.findings
                .insert("domain uses include!; source must stay in the scanned tree".into());
        }
        visit::visit_macro(self, item);
    }
}

fn redirects_module(meta: &syn::Meta) -> bool {
    if meta
        .path()
        .get_ident()
        .is_some_and(|id| id.unraw() == "path")
    {
        return true;
    }
    if let syn::Meta::List(list) = meta
        && list
            .path
            .get_ident()
            .is_some_and(|id| id.unraw() == "cfg_attr")
    {
        return list
            .parse_args_with(
                syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
            )
            .map_or(true, |args| args.iter().any(redirects_module));
    }
    false
}

fn inspect(source: &str, module: &[String]) -> Result<Vec<String>, syn::Error> {
    let syntax = syn::parse_file(source)?;
    let mut inspector = Inspector {
        module: module.to_vec(),
        findings: BTreeSet::new(),
    };
    inspector.visit_file(&syntax);
    Ok(inspector.findings.into_iter().collect())
}

fn check_domain_declaration(source: &str) -> Result<Vec<String>, syn::Error> {
    let mut errors = Vec::new();
    for item in syn::parse_file(source)?.items {
        if let syn::Item::Mod(module) = item
            && module.ident.unraw() == "domain"
            && (module.content.is_some() || module.attrs.iter().any(|a| redirects_module(&a.meta)))
        {
            errors.push(
                "domain declaration leaves scanned paths; review the layout and update its checks"
                    .into(),
            );
        }
    }
    Ok(errors)
}

fn check(files: &BTreeMap<String, String>, baseline: &BTreeMap<String, Baseline>) -> Vec<String> {
    let mut errors = Vec::new();
    for (file, source) in files {
        let path = file
            .strip_prefix("crates/yomibu-core/src/")
            .unwrap()
            .strip_suffix(".rs")
            .unwrap();
        let mut module: Vec<String> = path.split('/').map(String::from).collect();
        if module.last().map(String::as_str) == Some("mod") {
            module.pop();
        }
        let findings = match inspect(source, &module) {
            Ok(findings) => findings,
            Err(error) => {
                errors.push(format!("{file}: cannot parse Rust: {error}"));
                continue;
            }
        };
        if let Some(existing) = baseline.get(file) {
            if findings.is_empty() {
                errors.push(format!("{file}: stale baseline; remove the fixed entry"));
            } else if existing.sha256 != format!("{:x}", Sha256::digest(source.as_bytes()))
                || existing.violations != findings
            {
                errors.push(format!("{file}: baselined source or findings changed; resolve the violations or obtain explicit review"));
            }
        } else {
            errors.extend(
                findings
                    .into_iter()
                    .map(|finding| format!("{file}: {finding}")),
            );
        }
    }
    for file in baseline.keys().filter(|file| !files.contains_key(*file)) {
        errors.push(format!("{file}: stale baseline; remove the deleted entry"));
    }
    errors
}

fn collect_sources(root: &Path, directory: &Path, files: &mut BTreeMap<String, String>) {
    for entry in std::fs::read_dir(directory).expect("read domain directory") {
        let entry = entry.expect("read domain entry");
        let kind = entry.file_type().expect("read domain entry type");
        let path = entry.path();
        assert!(
            !kind.is_symlink(),
            "domain source must not escape the scanned tree: {}",
            path.display()
        );
        if kind.is_dir() {
            collect_sources(root, &path, files);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .to_str()
                .unwrap()
                .replace('\\', "/");
            files.insert(
                relative,
                std::fs::read_to_string(path).expect("read Rust source"),
            );
        }
    }
}

#[test]
fn repository_domain_respects_the_boundary_and_exact_baseline() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let domain = root.join("crates/yomibu-core/src/domain");
    let mut files = BTreeMap::new();
    let domain_file = domain.with_extension("rs");
    if domain_file.exists() {
        assert!(
            !domain_file.is_symlink(),
            "domain source must not be a symlink"
        );
        files.insert(
            "crates/yomibu-core/src/domain.rs".into(),
            std::fs::read_to_string(&domain_file).expect("read domain.rs"),
        );
    }
    if domain.exists() {
        assert!(!domain.is_symlink(), "domain source must not be a symlink");
        collect_sources(root, &domain, &mut files);
    }
    let baseline: BTreeMap<String, Baseline> = serde_json::from_str(
        &std::fs::read_to_string(root.join("tools/domain-baseline.json"))
            .expect("read domain baseline"),
    )
    .expect("parse domain baseline");
    let mut errors = check_domain_declaration(
        &std::fs::read_to_string(root.join("crates/yomibu-core/src/lib.rs"))
            .expect("read core lib.rs"),
    )
    .expect("parse core lib.rs");
    errors.extend(check(&files, &baseline));
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[test]
fn rejects_grouped_and_aliased_port_imports() {
    let source = "use crate::{domain::Value, ports::capabilities::{Selector as S, Embedder}};";
    let errors = inspect(source, &["domain".into()]).unwrap();
    assert_eq!(errors.len(), 2, "{errors:?}");
    assert!(
        errors
            .iter()
            .any(|e| e.contains("ports::capabilities::Selector"))
    );
}

#[test]
fn rejects_relative_paths_and_crate_aliases() {
    for source in [
        "use super::super::ports::Selector;",
        "use crate as root;",
        "use yomibu_core::ports as p;",
        "use crate::*;",
        "extern crate self as root; use root::ports::Selector;",
        "fn use_port(p: &dyn crate::ports::Selector) {}",
    ] {
        let errors = inspect(source, &["domain".into(), "selection".into()]).unwrap();
        assert!(!errors.is_empty(), "accepted: {source}");
    }
}

#[test]
fn rejects_re_exports_through_other_core_modules() {
    for source in [
        "pub use crate::ports::Selector as Helper;",
        "use crate::extension::ReexportedSelector;",
        "use crate::ReexportedSelector;",
    ] {
        assert!(!inspect(source, &["domain".into()]).unwrap().is_empty());
    }
}

#[test]
fn checks_nested_modules_and_inactive_cfg_branches() {
    let source = "#[cfg(windows)] mod nested { use super::super::ports::Selector; }";
    assert!(!inspect(source, &["domain".into()]).unwrap().is_empty());
}

#[test]
fn accepts_domain_paths_pure_libraries_and_visibility() {
    let source = r#"
        use crate::domain::{Value, knowledge::Observation};
        use crate::r#domain::Value;
        use super::Value as V;
        use std::collections::BTreeSet;
        use chrono::{DateTime, Utc};
        pub(crate) struct A;
        pub(in crate::domain) struct B;
        mod nested { use super::super::Value; }
        // use crate::ports::Selector;
        const TEXT: &str = "crate::ports::Selector";
    "#;
    assert!(
        inspect(source, &["domain".into(), "selection".into()])
            .unwrap()
            .is_empty()
    );
}

#[test]
fn rejects_source_inclusion_outside_the_scanned_tree() {
    for source in [
        "include!(\"../../outside.rs\");",
        "std::include!(\"../../outside.rs\");",
        "core::include!(\"../../outside.rs\");",
        "r#include!(\"../../outside.rs\");",
        "std::r#include!(\"../../outside.rs\");",
        "use std::include as load; load!(\"../../outside.rs\");",
        "#[path = \"../../outside.rs\"] mod outside;",
        "#[cfg_attr(windows, path = \"../../outside.rs\")] mod outside;",
    ] {
        assert!(!inspect(source, &["domain".into()]).unwrap().is_empty());
    }
}

#[test]
fn malformed_rust_is_not_a_pass() {
    assert!(inspect("pub fn {", &["domain".into()]).is_err());
}

#[test]
fn domain_layout_can_be_conventional_or_removed() {
    for source in ["pub mod domain; pub mod ports;", ""] {
        assert!(check_domain_declaration(source).unwrap().is_empty());
    }
}

#[test]
fn domain_layout_cannot_silently_leave_scanned_paths() {
    for source in [
        "pub mod domain { use crate::ports::Selector; }",
        "pub mod r#domain { use crate::ports::Selector; }",
        "#[r#path = \"other.rs\"] pub mod domain;",
        "#[path = \"other.rs\"] pub mod domain;",
        "#[cfg_attr(windows, path = \"other.rs\")] pub mod domain;",
    ] {
        assert!(
            !check_domain_declaration(source).unwrap().is_empty(),
            "{source}"
        );
    }
}

const FILE: &str = "crates/yomibu-core/src/domain/selection.rs";
const VIOLATION: &str = "use crate::ports::Selector;";

fn baseline_for(source: &str) -> BTreeMap<String, Baseline> {
    use sha2::{Digest, Sha256};
    BTreeMap::from([(
        FILE.into(),
        Baseline {
            sha256: format!("{:x}", Sha256::digest(source.as_bytes())),
            violations: vec!["domain depends on crate::ports::Selector".into()],
        },
    )])
}

#[test]
fn unbaselined_violation_fails() {
    let files = BTreeMap::from([(FILE.into(), VIOLATION.into())]);
    assert!(!check(&files, &BTreeMap::new()).is_empty());
}

#[test]
fn exact_existing_violation_is_accepted() {
    let files = BTreeMap::from([(FILE.into(), VIOLATION.into())]);
    assert!(check(&files, &baseline_for(VIOLATION)).is_empty());
}

#[test]
fn a_changed_baselined_file_fails_even_with_the_same_import() {
    let files = BTreeMap::from([(FILE.into(), format!("{VIOLATION}\nfn another_call() {{}}"))]);
    let errors = check(&files, &baseline_for(VIOLATION));
    assert!(errors.iter().any(|e| e.contains("changed")), "{errors:?}");
}

#[test]
fn baselined_violation_cannot_spread_to_another_file() {
    let files = BTreeMap::from([
        (FILE.into(), VIOLATION.into()),
        (
            "crates/yomibu-core/src/domain/other.rs".into(),
            VIOLATION.into(),
        ),
    ]);
    assert!(!check(&files, &baseline_for(VIOLATION)).is_empty());
}

#[test]
fn removed_or_fixed_violations_require_removing_the_baseline_entry() {
    for files in [
        BTreeMap::new(),
        BTreeMap::from([(FILE.into(), "pub struct Pure;".into())]),
    ] {
        let errors = check(&files, &baseline_for(VIOLATION));
        assert!(errors.iter().any(|e| e.contains("stale")), "{errors:?}");
    }
}

#[test]
fn baseline_must_list_the_actual_findings() {
    let files = BTreeMap::from([(FILE.into(), VIOLATION.into())]);
    let mut baseline = baseline_for(VIOLATION);
    baseline.get_mut(FILE).unwrap().violations.clear();
    assert!(!check(&files, &baseline).is_empty());
}
