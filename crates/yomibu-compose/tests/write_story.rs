use yomibu_compose::{BuildError, CompositionRoot, Configuration};

/// `story` comes first: TOML keys outside any table must precede the first table header.
fn configuration(story: &str, words: &str) -> String {
    format!("{story}\n[infrastructure.inventory]\nbackend = \"text_file\"\npath = \"{words}\"\n")
}

const ECHO: &str = "[[story.generator]]\ncomponent = \"echo\"\n";

fn parse(text: &str) -> Configuration {
    text.parse().expect("valid configuration")
}

#[test]
fn builds_and_runs_write_story_from_a_configuration() {
    let words =
        std::env::temp_dir().join(format!("yomibu-{}-compose-words.txt", std::process::id()));
    std::fs::write(&words, "猫\n犬\n").expect("words file written");
    let configuration = parse(&configuration(ECHO, &words.display().to_string()));
    let root = CompositionRoot::new(&configuration.infrastructure).expect("root built");

    let write_story = root
        .build_write_story(&configuration.story)
        .expect("operation built");
    let candidate = write_story.run().expect("story written");

    assert_eq!(candidate.text, "猫\n犬");
}

#[test]
fn rejects_text_that_is_not_a_configuration() {
    let Err(error) = "this is not toml".parse::<Configuration>() else {
        panic!("invalid text fails");
    };

    assert!(error.to_string().starts_with("invalid configuration: "));
}

#[test]
fn rejects_unknown_keys() {
    let text = configuration(&format!("colour = \"red\"\n{ECHO}"), "words.txt");

    let Err(error) = text.parse::<Configuration>() else {
        panic!("unknown key fails");
    };

    assert!(error.to_string().contains("colour"));
}

#[test]
fn rejects_an_unknown_inventory_backend() {
    let configuration = parse(&configuration(ECHO, "words.txt").replace("text_file", "parchment"));

    let Err(error) = CompositionRoot::new(&configuration.infrastructure) else {
        panic!("unknown backend fails");
    };

    assert!(matches!(error, BuildError::Unknown { ref name, .. } if name == "parchment"));
    assert_eq!(
        error.to_string(),
        "unknown inventory backend 'parchment'; known: text_file"
    );
}

#[test]
fn rejects_an_unknown_generator() {
    let configuration = parse(&configuration(&ECHO.replace("echo", "oracle"), "words.txt"));
    let root = CompositionRoot::new(&configuration.infrastructure).expect("root built");

    let Err(error) = root.build_write_story(&configuration.story) else {
        panic!("unknown generator fails");
    };

    assert!(matches!(error, BuildError::Unknown { ref name, .. } if name == "oracle"));
    assert_eq!(error.to_string(), "unknown generator 'oracle'; known: echo");
}

#[test]
fn each_table_reads_only_its_own_name_key() {
    let generator_named_by_backend =
        configuration("[[story.generator]]\nbackend = \"echo\"\n", "words.txt");
    let inventory_named_by_component =
        configuration(ECHO, "words.txt").replace("backend =", "component =");

    assert!(generator_named_by_backend.parse::<Configuration>().is_err());
    assert!(
        inventory_named_by_component
            .parse::<Configuration>()
            .is_err()
    );
}

#[test]
fn rejects_options_the_component_does_not_take() {
    let story = format!("{ECHO}volume = 11\n");
    let configuration = parse(&configuration(&story, "words.txt"));
    let root = CompositionRoot::new(&configuration.infrastructure).expect("root built");

    let Err(error) = root.build_write_story(&configuration.story) else {
        panic!("unknown option fails");
    };

    assert!(matches!(error, BuildError::InvalidOptions { name, .. } if name == "echo"));
}

#[test]
fn requires_exactly_one_generator() {
    let none = parse(&configuration("story = {}\n", "words.txt"));
    let two = parse(&configuration(&format!("{ECHO}\n{ECHO}"), "words.txt"));
    let root = CompositionRoot::new(&none.infrastructure).expect("root built");

    let Err(none_error) = root.build_write_story(&none.story) else {
        panic!("no generator fails");
    };
    let Err(two_error) = root.build_write_story(&two.story) else {
        panic!("two generators fail");
    };

    assert!(matches!(
        none_error,
        BuildError::GeneratorCount { found: 0 }
    ));
    assert!(matches!(two_error, BuildError::GeneratorCount { found: 2 }));
}
