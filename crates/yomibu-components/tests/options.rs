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
    use yomibu_components::{http_embeddings as http, openai_story_generation as openai};
    use yomibu_core::{
        component::options::{OptionError, Options},
        domain::story::StoryGenerationOptions,
    };
    let mut values = Options::default();
    assert!(
        matches!(http::model_identity(values.for_component(&http::COMPONENT), "local"),
        Err(OptionError::Missing { name, .. }) if name == http::MODEL.name)
    );
    values.set(http::MODEL, "embedding-model".into()).unwrap();
    values.set(http::REVISION, "pinned".into()).unwrap();
    values.set(http::DIMENSIONS, 2).unwrap();
    let identity = http::model_identity(values.for_component(&http::COMPONENT), "local").unwrap();
    assert_eq!(
        (
            identity.model.as_str(),
            identity.revision.as_str(),
            identity.dimensions
        ),
        ("embedding-model", "pinned", 2)
    );
    assert!(http::model_identity(values.for_component(&openai::COMPONENT), "local").is_err());
    assert!(http::has_model_options(values.for_component(&http::COMPONENT)).unwrap());

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
    assert!(openai::generation_options(values.for_component(&http::COMPONENT), defaults).is_err());
}

#[test]
fn local_encoder_construction_requires_its_scoped_endpoint_before_building_transport() {
    use yomibu_components::{http_embeddings as http, openai_story_generation as openai};
    use yomibu_core::component::options::{OptionError, Options};
    let mut values = Options::default();
    values.set(http::MODEL, "embedding-model".into()).unwrap();
    values.set(http::REVISION, "pinned".into()).unwrap();
    values.set(http::DIMENSIONS, 2).unwrap();
    let identity = http::model_identity(values.for_component(&http::COMPONENT), "local").unwrap();
    assert!(
        matches!(http::HttpEmbedder::local(values.for_component(&http::COMPONENT), identity.clone()),
        Err(http::BuildError::Options(OptionError::Missing { name, .. })) if name == http::ENDPOINT.name)
    );
    values.clear(http::ENDPOINT.setting()).unwrap();
    assert!(
        http::HttpEmbedder::local(values.for_component(&http::COMPONENT), identity.clone()).is_ok()
    );
    assert!(matches!(
        http::HttpEmbedder::local(values.for_component(&openai::COMPONENT), identity),
        Err(http::BuildError::Options(OptionError::Invalid { .. }))
    ));
}
