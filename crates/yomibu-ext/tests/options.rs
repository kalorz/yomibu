use std::collections::BTreeSet;
use yomibu_ext::COMPONENTS;

#[test]
fn registered_components_have_unique_names_and_owned_settings() {
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
                identifier(name.option) && flags.insert(name.key()),
                "{}",
                name.key()
            );
        }
    }
}

#[test]
fn providers_read_only_their_scoped_options_and_keep_domain_configuration_typed() {
    use yomibu_core::{domain::generation::StoryFormat, extension::options::Options};
    use yomibu_ext::{openai, wanikani};
    let mut values = Options::default();
    let options = openai::generation_options(
        values.for_component(&openai::COMPONENT),
        3,
        StoryFormat::Sentence,
    )
    .unwrap();
    assert_eq!(options.model, "gpt-6-luna");
    assert_eq!(options.candidate_count, 3);
    assert_eq!(options.format, StoryFormat::Sentence);
    values.set(openai::MODEL, "text-model".into());
    assert_eq!(
        openai::generation_options(
            values.for_component(&openai::COMPONENT),
            3,
            StoryFormat::Sentence
        )
        .unwrap()
        .model,
        "text-model"
    );
    assert!(
        openai::generation_options(
            values.for_component(&wanikani::COMPONENT),
            3,
            StoryFormat::Sentence
        )
        .is_err()
    );
}
