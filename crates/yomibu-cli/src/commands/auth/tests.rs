use super::*;
use std::{cell::RefCell, collections::BTreeMap};
use yomibu::application::Operation;
use yomibu::configuration::{ConfigurationInput, ProcessOverrides, components};

#[derive(Default)]
struct InMemoryStore {
    values: RefCell<BTreeMap<String, Secret>>,
    reads: RefCell<Vec<String>>,
}
impl Store for InMemoryStore {
    fn read(&self, key: &str) -> Result<Option<Secret>, CredentialError> {
        self.reads.borrow_mut().push(key.into());
        Ok(self.values.borrow().get(key).cloned())
    }
    fn write(&self, key: &str, value: &Secret) -> Result<()> {
        self.values.borrow_mut().insert(key.into(), value.clone());
        Ok(())
    }
}

fn configuration(application: &str, pipeline: &str) -> Configuration {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("config.toml"), application).unwrap();
    std::fs::write(dir.path().join("default-pipeline.toml"), pipeline).unwrap();
    Configuration::load(
        ConfigurationInput {
            data_dir: Some(dir.path().into()),
            config: None,
            home: None,
            environment: BTreeMap::new(),
            flags: ProcessOverrides::default(),
        },
        &Operation::Auth,
    )
    .unwrap()
}

#[test]
fn setup_deduplicates_shared_slots_and_promotes_required_uses() {
    let config = configuration(
        "",
        "[pipeline.selection]\nembeddings=true\n[pipeline.embedding]\nprovider='openai'\n",
    );
    let needs = missing_inputs(&config, &Credentials::default()).unwrap();
    assert_eq!(needs.len(), 2);
    let openai = needs.iter().find(|need| need.key == "openai").unwrap();
    assert!(openai.required);
    assert_eq!(openai.requirements.len(), 2);
    let store = InMemoryStore::default();
    let mut prompts = Vec::new();
    let mut output = Vec::new();
    setup(
        needs,
        &store,
        |need| {
            prompts.push((need.key.clone(), need.required));
            Ok(if need.required {
                Some("synthetic-openai".into())
            } else {
                None
            })
        },
        &mut output,
    )
    .unwrap();
    assert_eq!(
        prompts,
        [("openai".into(), true), ("wanikani".into(), false)]
    );
    assert_eq!(*store.reads.borrow(), ["openai", "wanikani"]);
    assert_eq!(store.values.borrow().len(), 1);
    assert_eq!(
        store.values.borrow()["openai"].expose().unwrap(),
        "synthetic-openai"
    );
    assert_eq!(
        String::from_utf8(output).unwrap(),
        "Saved openai in macOS Keychain.\nSkipped wanikani.\n"
    );
}

#[test]
fn disabled_or_local_integrations_do_not_request_credentials() {
    for provider in ["local", "lexical-baseline", "openai"] {
        let config = configuration(
            "[application]\nsync=false\n[application.credentials]\n\"http-embeddings.api-key\"={provider='supplied',key='http-embeddings.api-key'}\n",
            &format!(
                "[pipeline.embedding]\nprovider='{provider}'\n[pipeline.selection]\nembeddings=false\n"
            ),
        );
        let needs = missing_inputs(&config, &Credentials::default()).unwrap();
        assert_eq!(needs.len(), 1);
        assert_eq!(needs[0].key, "openai");
    }
    let config = configuration(
        "[application]\nsync=false\n",
        "[pipeline.selection]\nembeddings=true\n[pipeline.embedding]\nprovider='local'\n",
    );
    assert_eq!(
        missing_inputs(&config, &Credentials::default())
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn setup_skips_supplied_and_stored_credentials_and_can_skip_required_keys() {
    let config = configuration("", "");
    let mut credentials = Credentials::default();
    credentials.set_environment(components::GENERATION_KEY, "synthetic-env".into());
    let store = InMemoryStore::default();
    store
        .values
        .borrow_mut()
        .insert("wanikani".into(), "synthetic-stored".into());
    let mut output = Vec::new();
    setup(
        missing_inputs(&config, &credentials).unwrap(),
        &store,
        |_| panic!("Prompted for an available credential"),
        &mut output,
    )
    .unwrap();
    assert_eq!(*store.reads.borrow(), ["wanikani"]);
    assert_eq!(
        String::from_utf8(output).unwrap(),
        "No missing credentials to save.\n"
    );

    let mut output = Vec::new();
    setup(
        missing_inputs(&config, &Credentials::default()).unwrap(),
        &store,
        |_| Ok(None),
        &mut output,
    )
    .unwrap();
    assert_eq!(
        String::from_utf8(output).unwrap(),
        "Skipped openai; story generation still needs a credential.\n"
    );
}

#[test]
fn unavailable_store_stops_setup_before_prompting_or_writing() {
    struct UnavailableStore;
    impl Store for UnavailableStore {
        fn read(&self, _: &str) -> Result<Option<Secret>, CredentialError> {
            Err(CredentialError::StoreUnavailable)
        }
        fn write(&self, _: &str, _: &Secret) -> Result<()> {
            panic!("Wrote to an unavailable store");
        }
    }
    let mut output = Vec::new();
    let error = setup(
        vec![target("openai").unwrap()],
        &UnavailableStore,
        |_| panic!("Prompted after a store failure"),
        &mut output,
    )
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        CredentialError::StoreUnavailable.to_string()
    );
    assert!(output.is_empty());
}

#[test]
fn private_binding_reads_and_saves_only_its_own_slot() {
    let config = configuration(
        "[application]\nsync=false\n[application.credentials]\n\"openai-story-generation.api-key\"={provider='supplied',key='openai-story-generation.api-key'}\n",
        "",
    );
    let mut credentials = Credentials::default();
    credentials.set_shared_environment("openai", "synthetic-shared-env".into());
    let store = InMemoryStore::default();
    store
        .values
        .borrow_mut()
        .insert("openai".into(), "synthetic-shared-stored".into());
    let needs = missing_inputs(&config, &credentials).unwrap();
    assert_eq!(needs.len(), 1);
    let mut output = Vec::new();
    setup(
        needs,
        &store,
        |_| Ok(Some("synthetic-private".into())),
        &mut output,
    )
    .unwrap();
    assert_eq!(*store.reads.borrow(), ["openai-story-generation.api-key"]);
    assert_eq!(
        store.values.borrow()["openai"].expose().unwrap(),
        "synthetic-shared-stored"
    );
    assert_eq!(
        store.values.borrow()["openai-story-generation.api-key"]
            .expose()
            .unwrap(),
        "synthetic-private"
    );
}

#[test]
fn targeted_setup_replaces_one_slot_and_invalid_values_never_reach_storage() {
    let store = InMemoryStore::default();
    store
        .values
        .borrow_mut()
        .insert("openai".into(), "synthetic-old".into());
    let need = target("openai").unwrap();
    let mut output = Vec::new();
    save(
        &need,
        &store,
        |_| Ok(Some("synthetic-new".into())),
        &mut output,
    )
    .unwrap();
    assert!(store.reads.borrow().is_empty());
    for value in ["synthetic-secret\nInjected", "synthetic-secret\u{1b}", " "] {
        let error = save(&need, &store, |_| Ok(Some(value.into())), &mut output).unwrap_err();
        assert_eq!(
            error.to_string(),
            "API key must be nonblank printable ASCII without spaces."
        );
        assert_eq!(
            store.values.borrow()["openai"].expose().unwrap(),
            "synthetic-new"
        );
    }
    assert_eq!(
        String::from_utf8(output).unwrap(),
        "Saved openai in macOS Keychain.\n"
    );
}

#[test]
fn keychain_names_preserve_shared_and_component_boundaries() {
    assert_eq!(identity("openai"), ("yomibu:openai".into(), "api-key"));
    assert_eq!(identity("wanikani"), ("yomibu:wanikani".into(), "api-key"));
    assert_eq!(
        identity("openai-story-generation.api-key"),
        ("yomibu:openai-story-generation".into(), "api-key")
    );
    assert_eq!(
        identity("http-embeddings.api-key"),
        ("yomibu:http-embeddings".into(), "api-key")
    );
}

#[test]
fn auth_configuration_ignores_story_inputs_and_unused_model_options() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("config.toml"),
        "[application]\nsync=false\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("default-pipeline.toml"), "[pipeline]\nmodel=7\n[pipeline.selection]\nsteps=[]\nembeddings=true\n[pipeline.embedding]\nprovider='openai'\n").unwrap();
    std::fs::write(dir.path().join("default-story.toml"), "broken story TOML").unwrap();
    let config = Configuration::load(
        ConfigurationInput {
            data_dir: Some(dir.path().into()),
            config: None,
            home: None,
            environment: BTreeMap::new(),
            flags: ProcessOverrides::default(),
        },
        &Operation::Auth,
    )
    .unwrap();
    let needs = missing_inputs(&config, &Credentials::default()).unwrap();
    assert_eq!(needs.len(), 1);
    assert_eq!(needs[0].requirements.len(), 2);
}
