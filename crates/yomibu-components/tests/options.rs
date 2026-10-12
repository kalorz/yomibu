use std::collections::BTreeSet;
use yomibu_components::COMPONENTS;

#[test]
fn registered_components_have_unique_names_owned_settings_and_valid_defaults() {
    let mut ids = BTreeSet::new();
    let mut flags = BTreeSet::new();
    let identifier = |name: &str| {
        !name.is_empty()
            && name
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    };
    for component in COMPONENTS {
        assert!(
            identifier(component.id) && ids.insert(component.id),
            "{}",
            component.id
        );
        for &setting in component.settings {
            let name = setting.name();
            assert_eq!(name.component, component.id);
            assert!(
                identifier(name.option) && flags.insert(name.cli()),
                "{}",
                name.key()
            );
            setting.default_value().unwrap();
        }
    }
}

#[test]
fn providers_read_only_their_scoped_options_and_keep_domain_configuration_typed() {
    use yomibu_components::{openai_story_generation as openai, wanikani_source as wanikani};
    use yomibu_core::{component::options::Options, domain::story::StoryGenerationOptions};
    let mut values = Options::default();
    let defaults = StoryGenerationOptions {
        model: "fallback".into(),
        ..Default::default()
    };
    assert_eq!(
        openai::generation_options(values.for_component(&openai::COMPONENT), defaults.clone())
            .unwrap()
            .model,
        "fallback"
    );
    values.set(openai::MODEL, "text-model".into()).unwrap();
    assert_eq!(
        openai::generation_options(values.for_component(&openai::COMPONENT), defaults.clone())
            .unwrap()
            .model,
        "text-model"
    );
    assert!(
        openai::generation_options(values.for_component(&wanikani::COMPONENT), defaults).is_err()
    );
}
