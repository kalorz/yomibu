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
