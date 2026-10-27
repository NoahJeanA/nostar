//! Reference engine with a fixed dictionary excerpt against tests/cases.tsv (TECHNIK.md 14.1).

use nostar::config::{Config, Fallback, MarkerChar};
use nostar::rules::{self, build};
use nostar::source;

const FIXTURE: &str = include_str!("fixtures/dictionary.js");
const COMMIT: &str = "febca802f868c4ce968bbae70c099cdbc0454e18";

fn check(config: &Config) -> (usize, usize) {
    let mut lexicon = source::parse(FIXTURE, "dictionary.js", COMMIT).unwrap();
    lexicon.add_words(&config.words);
    let set = build::build(&lexicon, config);
    let cases = rules::cases(config).unwrap();
    let failures = rules::check(&set, &cases.cases);
    assert!(
        failures.is_empty(),
        "{} of {} cases differ{}",
        failures.len(),
        cases.cases.len(),
        rules::describe_failures(&failures)
    );
    (cases.cases.len(), cases.skipped)
}

#[test]
fn default_config_passes_all_cases() {
    let (checked, skipped) = check(&Config::default());
    assert!(checked > 100);
    // only cases for fallback "marker" and "off"
    assert_eq!(skipped, 2);
}

#[test]
fn other_fallbacks() {
    for fallback in [Fallback::Marker, Fallback::Off] {
        check(&Config {
            fallback,
            ..Config::default()
        });
    }
}

#[test]
fn fewer_markers_and_no_binnen_i() {
    check(&Config {
        markers: vec![MarkerChar::Star, MarkerChar::Underscore],
        binnen_i: false,
        ..Config::default()
    });
}

#[test]
fn own_words_win() {
    let config: Config = Config::parse(
        "[words.\"Ärzt\"]\nsingular = \"Doktor\"\nplural = \"Doktoren\"\ncompound = \"Doktoren\"\n\
         [words.Freund]\nsingular = \"Freund\"\nplural = \"Freunde\"\ncompound = \"Freundes\"\n",
        "config.toml".as_ref(),
    )
    .unwrap();
    check(&config);
    let mut lexicon = source::parse(FIXTURE, "dictionary.js", COMMIT).unwrap();
    lexicon.add_words(&config.words);
    let set = build::build(&lexicon, &config);
    assert_eq!(set.render("Die Ärzt*innen"), "Die Doktoren");
    assert_eq!(
        set.render("Freund*innenkreis, Freund*innen"),
        "Freundeskreis, Freunde"
    );
}
