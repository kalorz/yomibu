use yomibu_core::capabilities::options::{Component, OptionKey, Options, Secret, Setting, Value};

const MODEL: OptionKey<String> = OptionKey::new("test", "model", "Model");
const SIZE: OptionKey<usize> = OptionKey::new("test", "size", "Size").validate(|value| {
    if (1..=8).contains(value) {
        Ok(())
    } else {
        Err("Size must be between 1 and 8.")
    }
});
const KEY: OptionKey<Secret> = OptionKey::new("test", "api-key", "API key");
const COMPONENT: Component = Component {
    id: "test",
    settings: &[MODEL.setting(), SIZE.setting(), KEY.setting()],
};

#[test]
fn typed_keys_parse_validate_and_read_from_shared_scoped_options() {
    let mut options = Options::default();
    options
        .insert(MODEL.setting(), MODEL.setting().parse("日本語").unwrap())
        .unwrap();
    options
        .insert(SIZE.setting(), SIZE.setting().parse("4").unwrap())
        .unwrap();
    assert_eq!(
        options
            .for_component(&COMPONENT)
            .get(MODEL)
            .unwrap()
            .map(String::as_str),
        Some("日本語")
    );
    assert_eq!(
        options.for_component(&COMPONENT).get(SIZE).unwrap(),
        Some(&4)
    );
    for invalid in ["0", "9", "-1", "not-an-integer"] {
        assert!(SIZE.setting().parse(invalid).is_err());
    }
    assert!(
        options
            .insert(SIZE.setting(), Value::String("4".into()))
            .is_err()
    );
    assert!(options.set(SIZE, 9).is_err());
    assert_eq!(
        options.for_component(&COMPONENT).get(SIZE).unwrap(),
        Some(&4)
    );
    let wrong_type = OptionKey::<String>::new("test", "size", "Wrong type");
    let wrong_component = OptionKey::<usize>::new("other", "size", "Wrong component");
    assert!(options.for_component(&COMPONENT).get(wrong_type).is_err());
    assert!(
        options
            .for_component(&COMPONENT)
            .get(wrong_component)
            .is_err()
    );
}

#[test]
fn values_are_per_invocation_and_secret_values_are_redacted() {
    let mut options = Options::default();
    options.set(MODEL, "first".into()).unwrap();
    options.set(KEY, Secret::from("synthetic-private")).unwrap();
    let mut next = options.clone();
    next.set(MODEL, "second".into()).unwrap();
    assert_eq!(
        options.get(MODEL).unwrap().map(String::as_str),
        Some("first")
    );
    assert_eq!(next.get(MODEL).unwrap().map(String::as_str), Some("second"));
    assert_eq!(
        next.get(KEY).unwrap().unwrap().expose().unwrap(),
        "synthetic-private"
    );
    assert!(!format!("{next:?}").contains("synthetic-private"));
    assert!(matches!(KEY.setting(), Setting::Secret(_)));
}
