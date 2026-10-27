//! Patch and check of the fixture fonts (TECHNIK.md 14.1).

use nostar::config::Config;
use nostar::font::{self, glyphs::Glyphs, verify::Shaper};
use nostar::lexicon::Lexicon;
use nostar::plan::{self, Plan};
use nostar::rules::build;
use nostar::rules::engine::RuleSet;
use nostar::source;
use read_fonts::{FontRef, TableProvider};
use write_fonts::FontBuilder;
use write_fonts::types::Tag;

const NOTO: &[u8] = include_bytes!("fixtures/NotoSans-Latin.ttf");
const ADWAITA: &[u8] = include_bytes!("fixtures/AdwaitaSans-Latin.ttf");
const RUBIK: &[u8] = include_bytes!("fixtures/Rubik-Latin.ttf");
const DICTIONARY: &str = include_str!("fixtures/dictionary.js");
const COMMIT: &str = "febca802f868c4ce968bbae70c099cdbc0454e18";

fn rules(lexicon: &Lexicon, config: &Config) -> (RuleSet, Plan) {
    let set = build::build(lexicon, config);
    let plan = plan::plan(set.rules());
    (set, plan)
}

fn fixture_rules(config: &Config) -> (RuleSet, Plan) {
    let lexicon = source::parse(DICTIONARY, "dictionary.js", COMMIT).unwrap();
    rules(&lexicon, config)
}

/// Patch passes the built-in check (cases and unchanged corpus); a few extra texts.
fn patch_and_check(original: &[u8]) -> Vec<u8> {
    let config = Config::default();
    let (set, plan) = fixture_rules(&config);
    let patched = font::patch(original, &plan, &config).unwrap();
    let copy = Shaper::new(&patched.data).unwrap();
    let orig = Shaper::new(original).unwrap();
    for text in [
        "Die Ärzt*innen und jede*r Kolleg*in",
        "Influencer*innen im Kaffeeflugzeug",
        "MitarbeiterInnen, Kolleg*INNEN, Oberärzt_innenkammer",
    ] {
        assert_eq!(
            copy.shape(text).unwrap(),
            orig.shape(&set.render(text)).unwrap(),
            "{text}"
        );
    }
    patched.data
}

#[test]
fn noto_with_ligatures() {
    let data = patch_and_check(NOTO);
    let font = FontRef::new(&data).unwrap();
    // the original lookups are still there, behind the new ones
    let lookups = font.gsub().unwrap().lookup_list().unwrap().lookup_count();
    let before = FontRef::new(NOTO)
        .unwrap()
        .gsub()
        .unwrap()
        .lookup_list()
        .unwrap()
        .lookup_count();
    assert!(lookups > before);
}

#[test]
fn variable_font_with_own_calt() {
    patch_and_check(ADWAITA);
}

#[test]
fn variable_font_with_feature_variations() {
    patch_and_check(RUBIK);
}

#[test]
fn font_without_gsub() {
    let font = FontRef::new(NOTO).unwrap();
    let mut b = FontBuilder::new();
    for record in font.table_directory().table_records() {
        let tag = record.tag();
        if tag != Tag::new(b"GSUB")
            && let Some(data) = font.table_data(tag)
        {
            b.add_raw(tag, data.as_bytes().to_vec());
        }
    }
    let without = b.build();
    assert!(FontRef::new(&without).unwrap().gsub().is_err());
    patch_and_check(&without);
}

/// Every dictionary form in the variants of TECHNIK.md 8.2 shapes as the engine says.
/// `NOSTAR_TEST_DICTIONARY=<path to dictionary.js> cargo test --release -- --ignored`
#[test]
#[ignore]
fn font_with_dictionary() {
    let path = std::env::var("NOSTAR_TEST_DICTIONARY").expect("NOSTAR_TEST_DICTIONARY");
    let text = std::fs::read_to_string(path).unwrap();
    let lexicon = source::parse(&text, "dictionary.js", COMMIT).unwrap();
    let config = Config::default();
    let (set, plan) = rules(&lexicon, &config);
    let texts = dictionary_forms(&lexicon);
    for (name, original) in [("noto", NOTO), ("adwaita", ADWAITA), ("rubik", RUBIK)] {
        let patched = font::patch(original, &plan, &config).unwrap();
        let copy = Shaper::new(&patched.data).unwrap();
        let orig = Shaper::new(original).unwrap();
        let mut failures = Vec::new();
        for t in &texts {
            if copy.shape(t).unwrap() != orig.shape(&set.render(t)).unwrap() {
                failures.push(t.clone());
            }
        }
        eprintln!(
            "{name}: {} forms, {} differ, lookups={} rules={} dropped={} size=+{}KiB",
            texts.len(),
            failures.len(),
            patched.lookups,
            patched.rules,
            patched.dropped,
            (patched.data.len() - original.len()) / 1024
        );
        assert!(
            failures.is_empty(),
            "{name}: {:?}",
            &failures[..failures.len().min(20)]
        );
    }
}

/// Every dictionary form in the variants of TECHNIK.md 8.2.
fn dictionary_forms(lexicon: &Lexicon) -> Vec<String> {
    let mut texts = Vec::new();
    for p in &lexicon.persons {
        let s = &p.stem;
        let lower = {
            let mut c = s.chars();
            c.next()
                .map(|f| f.to_lowercase().collect::<String>() + c.as_str())
                .unwrap_or_default()
        };
        texts.extend([
            format!("{s}*innen"),
            format!("{s}*in"),
            format!("{s}:innen"),
            format!("{s}/-in"),
            format!("{s}Innen"),
            format!("{s}In"),
            format!("{s}_innenkammer"),
            format!("Ober{lower}*innen"),
            format!("Ober{lower}*in"),
            format!("{}*INNEN", s.to_uppercase()),
            format!("{s}*Innen"),
        ]);
    }
    for w in &lexicon.adjectival {
        texts.push(format!("{w}*r"));
        texts.push(format!("{w}*n"));
    }
    for e in &lexicon.endings {
        texts.push(format!("Xyzab{}*innen", e.ending));
        texts.push(format!("Xyzab{}*in", e.ending));
    }
    texts
}

/// Patched fonts and texts for `tools/bench.sh` (TECHNIK.md 14.1):
/// `NOSTAR_TEST_DICTIONARY`, `NOSTAR_BENCH_FONTS` (paths separated by ':'),
/// `NOSTAR_BENCH_OUT` (output directory).
#[test]
#[ignore]
fn bench_fonts() {
    let var = |name| std::env::var(name).unwrap_or_else(|_| panic!("{name} not set"));
    let out = std::path::PathBuf::from(var("NOSTAR_BENCH_OUT"));
    std::fs::create_dir_all(&out).unwrap();
    let text = std::fs::read_to_string(var("NOSTAR_TEST_DICTIONARY")).unwrap();
    let lexicon = source::parse(&text, "dictionary.js", COMMIT).unwrap();
    let config = Config::default();
    let (set, plan) = rules(&lexicon, &config);
    let cases = nostar::rules::cases(&config).unwrap().cases;
    let write = |file: String, lines: &mut dyn Iterator<Item = String>| {
        let s: String = lines.map(|l| l + "\n").collect();
        std::fs::write(out.join(file), s).unwrap();
    };
    for path in var("NOSTAR_BENCH_FONTS").split(':') {
        let original = std::fs::read(path).unwrap();
        let patched = font::patch(&original, &plan, &config).unwrap();
        let name = std::path::Path::new(path).file_name().unwrap();
        std::fs::write(out.join(name), &patched.data).unwrap();
        eprintln!(
            "{}: lookups={} size=+{}KiB",
            name.to_string_lossy(),
            patched.lookups,
            (patched.data.len() - original.len()) / 1024
        );
        // as in font::patch: cases with a character the font lacks are left out
        let glyphs = Glyphs::new(&FontRef::new(&original).unwrap()).unwrap();
        let usable: Vec<_> = cases
            .iter()
            .filter(|c| c.input.chars().all(|ch| glyphs.has(ch)))
            .collect();
        let stem = std::path::Path::new(path)
            .file_stem()
            .unwrap()
            .to_string_lossy();
        write(
            format!("{stem}.cases_in.txt"),
            &mut usable.iter().map(|c| c.input.clone()),
        );
        write(
            format!("{stem}.cases_exp.txt"),
            &mut usable.iter().map(|c| c.expected.clone()),
        );
    }
    // Dictionary forms with the display the engine expects.
    let forms = dictionary_forms(&lexicon);
    write("forms_in.txt".into(), &mut forms.iter().cloned());
    write(
        "forms_exp.txt".into(),
        &mut forms.iter().map(|f| set.render(f)),
    );
    // marked.txt: prose, every second line with dictionary forms (TECHNIK.md 14.1).
    let prose = include_str!("bench/prose.txt");
    let stems: Vec<&str> = lexicon.persons.iter().map(|p| p.stem.as_str()).collect();
    let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = |n: usize| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % n as u64) as usize
    };
    let mut marked = String::new();
    for (i, line) in prose.lines().cycle().enumerate() {
        if marked.len() > 1_000_000 {
            break;
        }
        if i % 2 == 0 {
            let (a, b, c) = (
                stems[next(stems.len())],
                stems[next(stems.len())],
                stems[next(stems.len())],
            );
            marked.push_str(&format!(
                "Die {a}*innen und {b}:innen trafen sich mit jede*r {c}_in im Rathaus.\n"
            ));
        } else {
            marked.push_str(line);
            marked.push('\n');
        }
    }
    std::fs::write(out.join("marked.txt"), marked).unwrap();
}
