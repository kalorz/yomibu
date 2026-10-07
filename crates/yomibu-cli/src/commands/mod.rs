use crate::{
    args::{Cli, Command, DictionaryCommand},
    output,
};
use anyhow::{Context, Result};
use std::{
    collections::BTreeMap,
    io,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
use yomibu::app::{
    config::{Configuration, ConfigurationInput, ENVIRONMENT_SETTINGS, Settings},
    local::{Credentials, LocalApp, ServiceEndpoints},
};

enum Action {
    Story,
    Preview,
    Retrieval,
    Analyze(PathBuf),
    Import(PathBuf),
    Verify,
    Sync,
    Status,
}

pub(crate) fn run(cli: Cli, endpoints: ServiceEndpoints) -> Result<()> {
    let Some(command) = cli.command else {
        return Ok(());
    };
    let (action, flags) = match command {
        Command::Story(args) => (Action::Story, args.settings()),
        Command::PreviewStory(args) => (Action::Preview, args.settings()),
        Command::PrepareRetrieval(args) => (Action::Retrieval, args.settings()),
        Command::Analyze { dictionary, input } => (Action::Analyze(input), dictionary.settings()),
        Command::Dictionary {
            command:
                DictionaryCommand::Import {
                    bundle,
                    dictionary_dir,
                },
        } => (
            Action::Import(bundle),
            Settings {
                dictionary_dir,
                ..Default::default()
            },
        ),
        Command::Dictionary {
            command: DictionaryCommand::Verify { dictionary_dir },
        } => (
            Action::Verify,
            Settings {
                dictionary_dir,
                ..Default::default()
            },
        ),
        Command::Sync => (Action::Sync, Settings::default()),
        Command::Status => (Action::Status, Settings::default()),
    };
    let environment: BTreeMap<_, _> = ENVIRONMENT_SETTINGS
        .iter()
        .filter_map(|name| {
            std::env::var(name)
                .ok()
                .map(|value| ((*name).into(), value))
        })
        .collect();
    let config = Configuration::load(ConfigurationInput {
        data_dir: cli
            .data_dir
            .or_else(|| std::env::var_os("YOMIBU_DATA_DIR").map(PathBuf::from)),
        config: cli
            .config
            .or_else(|| std::env::var_os("YOMIBU_CONFIG").map(PathBuf::from)),
        home: std::env::var_os("HOME")
            .filter(|home| !home.is_empty())
            .map(PathBuf::from),
        environment,
        flags,
    })?;
    let credentials = Credentials::new(
        cli.wanikani_api_key
            .or_else(|| std::env::var("YOMIBU_WANIKANI_API_KEY").ok()),
        cli.openai_api_key
            .or_else(|| std::env::var("YOMIBU_OPENAI_API_KEY").ok()),
    );
    let app = LocalApp::new(config, credentials).with_endpoints(endpoints);
    let clock = SystemTime::now();
    let seed = clock
        .duration_since(UNIX_EPOCH)
        .context("System clock")?
        .as_nanos() as u64;
    let mut out = io::stdout().lock();
    let mut err = io::stderr().lock();
    match action {
        Action::Story => {
            let mut progress_error = None;
            // Managed roots follow the importer's immutable-generation contract.
            let report = runtime()?.block_on(unsafe {
                app.story(clock.into(), seed, |event| {
                    if cli.verbose {
                        let result = if cli.json {
                            output::story::write_progress(&mut err, &event)
                        } else {
                            output::story::write_progress(&mut out, &event)
                        };
                        if let Err(error) = result {
                            progress_error = Some(error);
                        }
                    }
                })
            })?;
            if let Some(error) = progress_error {
                return Err(error.into());
            }
            output::story::write_run(&mut out, &mut err, &report, cli.json, cli.verbose)?;
        }
        Action::Preview => output::story::write_preview(
            &mut out,
            &mut err,
            &app.preview(clock.into(), seed)?,
            cli.json,
        )?,
        Action::Retrieval => {
            let cache = runtime()?.block_on(app.prepare_retrieval(clock.into()))?;
            if cli.json {
                output::write_json(&mut out, &cache)?;
            } else {
                output::story::write_retrieval(&mut out, &cache)?;
            }
        }
        Action::Analyze(input) => {
            // Managed roots follow the importer's immutable-generation contract.
            let report = unsafe { app.analyze(&input) }?;
            if cli.json {
                output::write_json(&mut out, &report)?;
            } else {
                output::analysis::write_text(&mut out, &report)?;
            }
        }
        Action::Import(bundle) => {
            output::dictionary::write_import(&mut out, &app.import_dictionary(&bundle)?, cli.json)?
        }
        Action::Verify => {
            app.verify_dictionary()?;
            output::dictionary::write_verified(&mut out, cli.json)?;
        }
        Action::Sync => {
            let report = runtime()?.block_on(app.sync())?;
            if cli.json {
                output::write_json(&mut out, &report)?;
            } else {
                output::status::write_status(&mut out, &report.summary)?;
            }
        }
        Action::Status => {
            let summary = app.status()?;
            if cli.json {
                output::write_json(&mut out, &summary)?;
            } else {
                output::status::write_status(&mut out, &summary)?;
            }
        }
    }
    Ok(())
}
fn runtime() -> Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("I/O runtime")
}
