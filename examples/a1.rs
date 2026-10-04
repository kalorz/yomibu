//! Offline synthetic evaluation harness. No learner cache, service, or model access.
use std::{
    collections::HashSet,
    io::Read,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use clap::Parser;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use yomibu::{
    adapters::sudachi::SudachiAnalyzer,
    analysis::Sentence,
    evaluation::{CheckState, EvaluationBindings, REPORT_NOTICE, evaluate},
    grammar::GrammarDeclarations,
};

const MAX_PACKET_BYTES: usize = 1024 * 1024;

#[derive(Parser)]
#[command(about = "Run bounded A1 checks on an explicitly supplied synthetic packet")]
struct Arguments {
    dictionary: PathBuf,
    packet: PathBuf,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Packet {
    format_version: u8,
    synthetic: bool,
    reuse: String,
    cases: Vec<Case>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    sentence: String,
    grammar: Vec<String>,
    bindings: EvaluationBindings,
}

fn main() -> Result<()> {
    let args = Arguments::parse();
    let mut input = String::new();
    std::fs::File::open(&args.packet)
        .context("Cannot open the explicitly supplied synthetic packet")?
        .take(MAX_PACKET_BYTES as u64 + 1)
        .read_to_string(&mut input)?;
    let report = run_packet(&args.dictionary, &input)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    if report["execution_errors"].as_u64().unwrap_or(1) != 0 {
        bail!("Evaluation run contains execution errors; see JSON report.");
    }
    Ok(())
}

fn run_packet(dictionary: &Path, input: &str) -> Result<Value> {
    if input.len() > MAX_PACKET_BYTES {
        bail!("Synthetic packet exceeds 1 MiB.");
    }
    let packet: Packet = serde_json::from_str(input).context("Invalid synthetic packet")?;
    if packet.format_version != 1 || !packet.synthetic || packet.reuse.trim().is_empty() {
        bail!("Packet requires format_version 1, synthetic true, and explicit reuse terms.");
    }
    if packet.cases.is_empty() || packet.cases.len() > 60 {
        bail!("Packet must contain 1–60 cases.");
    }
    let mut ids = HashSet::new();
    for case in &packet.cases {
        if case.id.trim().is_empty() || !ids.insert(&case.id) {
            bail!("Case IDs must be nonblank and unique.");
        }
    }
    let analyzer = SudachiAnalyzer::load(dictionary);
    let mut reports = Vec::with_capacity(packet.cases.len());
    let mut errors = 0;
    for case in &packet.cases {
        let result = match &analyzer {
            Ok(analyzer) => run_case(analyzer, case),
            Err(error) => Err(anyhow::anyhow!("Dictionary initialization: {error}")),
        };
        reports.push(match result {
            Ok(report) => report,
            Err(error) => {
                errors += 1;
                json!({"input": case, "outcome": CheckState::NotRun, "checks": CheckState::NotRun, "execution_error": format!("{error:#}")})
            }
        });
    }
    Ok(
        json!({"notice": REPORT_NOTICE, "reference_evidence": "Not assessed by this executable", "execution_errors": errors, "cases": reports}),
    )
}

fn run_case(analyzer: &SudachiAnalyzer, case: &Case) -> Result<Value> {
    let sentence = Sentence::new(&case.sentence).context("Sentence input")?;
    let grammar = GrammarDeclarations::from_descriptions(case.grammar.iter().cloned())
        .context("Grammar input")?;
    let analysis = analyzer.analyze(sentence).context("Analyzer execution")?;
    let evaluation =
        evaluate(&analysis, &grammar, &case.bindings).context("Evaluation bindings or analysis")?;
    Ok(
        json!({"input": case, "analysis": analysis, "outcome": evaluation.outcome(), "evaluation": evaluation, "execution_error": null}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    const SMOKE: &str = include_str!("../tests/fixtures/a1/smoke.json");

    #[test]
    fn reports_completed_judgments_without_calling_them_accepted_exercises() {
        let dictionary = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/a1/system_core.dic");
        let report = run_packet(&dictionary, SMOKE).unwrap();
        assert_eq!(report["execution_errors"], 0);
        for (case, expected) in
            report["cases"]
                .as_array()
                .unwrap()
                .iter()
                .zip(["Pass", "Fail", "Inconclusive"])
        {
            assert_eq!(case["outcome"]["Completed"], expected);
            assert!(case["execution_error"].is_null());
            assert!(case["analysis"]["units"].is_array());
            for check in ["vocabulary", "inflection", "particles", "nominal", "scope"] {
                assert!(case["evaluation"][check]["state"]["Completed"].is_string());
            }
        }
        assert!(
            report["notice"]
                .as_str()
                .unwrap()
                .contains("not accepted exercises")
        );
        assert_eq!(
            report["reference_evidence"],
            "Not assessed by this executable"
        );
    }

    #[test]
    fn unavailable_dictionary_is_an_execution_error_and_checks_are_not_run() {
        let directory = tempfile::tempdir().unwrap();
        let report = run_packet(&directory.path().join("missing.dic"), SMOKE).unwrap();
        assert_eq!(report["execution_errors"], 3);
        for case in report["cases"].as_array().unwrap() {
            assert_eq!(case["outcome"], "NotRun");
            assert!(case["execution_error"].is_string());
            assert_eq!(case["checks"], "NotRun");
            assert!(case["evaluation"].is_null());
        }
    }
    #[test]
    fn invalid_packet_metadata_is_rejected_before_dictionary_work() {
        let directory = tempfile::tempdir().unwrap();
        let dictionary = directory.path().join("missing.dic");
        for (field, value) in [
            ("format_version", json!(2)),
            ("synthetic", json!(false)),
            ("reuse", json!(" ")),
            ("cases", json!([])),
        ] {
            let mut packet: Value = serde_json::from_str(SMOKE).unwrap();
            packet[field] = value;
            assert!(
                run_packet(&dictionary, &packet.to_string()).is_err(),
                "{field}"
            );
        }
        let mut packet: Value = serde_json::from_str(SMOKE).unwrap();
        packet["cases"][1]["id"] = packet["cases"][0]["id"].clone();
        assert!(run_packet(&dictionary, &packet.to_string()).is_err());
        packet["cases"] = json!(vec![packet["cases"][0].clone(); 61]);
        assert!(run_packet(&dictionary, &packet.to_string()).is_err());
        assert!(run_packet(&dictionary, &" ".repeat(1024 * 1024 + 1)).is_err());
    }

    #[test]
    fn one_invalid_case_does_not_erase_completed_results_or_its_error_cause() {
        let dictionary = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/a1/system_core.dic");
        let mut packet: Value = serde_json::from_str(SMOKE).unwrap();
        packet["cases"][1]["sentence"] = json!("猫".repeat(101));
        let report = run_packet(&dictionary, &packet.to_string()).unwrap();
        assert_eq!(report["execution_errors"], 1);
        assert_eq!(report["cases"][0]["outcome"]["Completed"], "Pass");
        assert_eq!(report["cases"][1]["outcome"], "NotRun");
        assert!(
            report["cases"][1]["execution_error"]
                .as_str()
                .unwrap()
                .contains("101 Unicode characters")
        );
        assert_eq!(report["cases"][2]["outcome"]["Completed"], "Inconclusive");
    }
}
