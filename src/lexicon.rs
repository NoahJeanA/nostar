//! Dictionary model (TECHNIK.md 7.3) plus own words from `[words]`.

use std::collections::BTreeMap;

use crate::config::Word;

/// Person noun: feminine stem and the masculine target forms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Person {
    /// As written before the marker: "Ärzt"
    pub stem: String,
    pub singular: String,
    pub plural: String,
    /// Form inside compounds; defaults to `plural`.
    pub compound: String,
    pub origin: Origin,
}

/// Ending rule: a stem ending in `ending` becomes `singular` or `plural`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndingRule {
    pub ending: String,
    pub singular: String,
    pub plural: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    Entgendert { commit: String },
    Config,
    Builtin,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Lexicon {
    pub persons: Vec<Person>,
    pub endings: Vec<EndingRule>,
    /// Nominalised adjectives such as "Vorsitzende".
    pub adjectival: Vec<String>,
}

impl Lexicon {
    /// Own words replace dictionary persons with the same stem; new stems are appended.
    pub fn add_words(&mut self, words: &BTreeMap<String, Word>) {
        let mut remaining: BTreeMap<&str, &Word> =
            words.iter().map(|(k, v)| (k.as_str(), v)).collect();
        for p in &mut self.persons {
            if let Some(w) = remaining.remove(p.stem.as_str()) {
                *p = person(&p.stem, w);
            }
        }
        self.persons
            .extend(remaining.into_iter().map(|(stem, w)| person(stem, w)));
    }
}

fn person(stem: &str, w: &Word) -> Person {
    Person {
        stem: stem.to_string(),
        singular: w.singular.clone(),
        plural: w.plural.clone(),
        compound: w.compound.clone(),
        origin: Origin::Config,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(singular: &str, plural: &str, compound: &str) -> Word {
        Word {
            singular: singular.into(),
            plural: plural.into(),
            compound: compound.into(),
        }
    }

    #[test]
    fn own_words_replace_and_extend() {
        let entgendert = Origin::Entgendert {
            commit: "febca802f868c4ce968bbae70c099cdbc0454e18".into(),
        };
        let mut lex = Lexicon {
            persons: vec![
                Person {
                    stem: "Freund".into(),
                    singular: "Freund".into(),
                    plural: "Freunde".into(),
                    compound: "Freunde".into(),
                    origin: entgendert.clone(),
                },
                Person {
                    stem: "Kund".into(),
                    singular: "Kunde".into(),
                    plural: "Kunden".into(),
                    compound: "Kunden".into(),
                    origin: entgendert.clone(),
                },
            ],
            ..Lexicon::default()
        };
        let words = BTreeMap::from([
            ("Freund".to_string(), word("Freund", "Freunde", "Freundes")),
            ("Ärzt".to_string(), word("Arzt", "Ärzte", "Ärzte")),
        ]);
        lex.add_words(&words);
        let stems: Vec<_> = lex.persons.iter().map(|p| p.stem.as_str()).collect();
        assert_eq!(stems, ["Freund", "Kund", "Ärzt"]);
        assert_eq!(lex.persons[0].compound, "Freundes");
        assert_eq!(lex.persons[0].origin, Origin::Config);
        assert_eq!(lex.persons[1].origin, entgendert);
        assert_eq!(lex.persons[2].singular, "Arzt");
    }
}
