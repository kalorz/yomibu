use yomibu_core::extension::{
    Component,
    options::{OptionError, OptionKey, Options, Setting, Value},
};
use yomibu_core::ports::infrastructure::credentials::Secret;

const MODEL: OptionKey<String> = OptionKey::new("test", "model", "Model");
const KEY: OptionKey<Secret> = OptionKey::new("test", "api-key", "API key");
const COMPONENT: Component = Component {
    id: "test",
    settings: &[MODEL.setting(), KEY.setting()],
};

#[test]
fn typed_keys_parse_and_read_from_shared_scoped_options() {
    let mut options = Options::default();
    options
        .insert(MODEL.setting(), MODEL.setting().parse("日本語").unwrap())
        .unwrap();
    assert_eq!(
        options
            .for_component(&COMPONENT)
            .get(MODEL)
            .unwrap()
            .map(String::as_str),
        Some("日本語")
    );
    assert!(serde_json::from_str::<Value>("42").is_err());
}

#[test]
fn values_are_per_invocation_and_clear_does_not_change_the_base() {
    let mut options = Options::default();
    options.set(MODEL, "first".into());
    let mut next = options.clone();
    next.set(MODEL, "second".into());
    assert_eq!(next.get(MODEL).unwrap().map(String::as_str), Some("second"));
    next.clear(MODEL.setting());
    assert!(next.get(MODEL).unwrap().is_none());
    assert_eq!(
        options.get(MODEL).unwrap().map(String::as_str),
        Some("first")
    );
    assert!(matches!(KEY.setting(), Setting::Secret(_)));
}

#[test]
fn ordinary_options_reject_credential_values_without_reflecting_them() {
    let token = "synthetic-private";
    let error = KEY.setting().parse(token).unwrap_err();
    assert!(!format!("{error:?} {error}").contains(token));
    assert!(!format!("{:?}", Secret::from(token)).contains(token));
    assert!(
        Options::default()
            .insert(KEY.setting(), Value::String(token.into()))
            .is_err()
    );
}

#[test]
fn required_reads_name_missing_settings_and_keep_scope_and_type_checks() {
    let mut values = Options::default();
    let error = values
        .for_component(&COMPONENT)
        .required(MODEL)
        .unwrap_err();
    assert!(matches!(error, OptionError::Missing { name, .. } if name == MODEL.name));
    assert_eq!(error.to_string(), "Missing test.model. Model");
    assert!(
        values
            .for_component(&COMPONENT)
            .get(MODEL)
            .unwrap()
            .is_none()
    );

    values.set(MODEL, "selected".into());
    const OTHER: OptionKey<String> = OptionKey::new("other", "model", "Other model");
    values.set(OTHER, "other".into());
    let options = values.for_component(&COMPONENT);
    assert_eq!(options.required(MODEL).unwrap(), "selected");
    for key in [OTHER, OptionKey::new("test", "api-key", "Wrong type")] {
        assert!(matches!(
            options.required(key),
            Err(OptionError::Invalid { .. })
        ));
    }
}
