//! `nostar rules build` and `nostar rules test`

use std::process::ExitCode;

use crate::Error;
use crate::config::Config;
use crate::fsutil::Lock;
use crate::paths::Paths;
use crate::rules::engine::RuleSet;
use crate::rules::{self, Markers, RulesError, build, store};
use crate::source;

pub fn build(paths: &Paths, config: &Config) -> Result<ExitCode, Error> {
    let _lock = Lock::acquire(&paths.lock())?;
    let mut lexicon = source::load(paths, &config.source)?;
    lexicon.add_words(&config.words);
    let set = build::build(&lexicon, config);
    let cases = rules::cases(config)?;
    let failures = rules::check(&set, &cases.cases);
    if !failures.is_empty() {
        return Err(RulesError::CasesDiffer {
            failed: failures.len(),
            total: cases.cases.len(),
            details: rules::describe_failures(&failures),
        }
        .into());
    }
    log::info!(
        "{} cases ok, {} skipped for this config",
        cases.cases.len(),
        cases.skipped
    );
    store::write(&paths.rules(), set.rules(), config).map_err(RulesError::from)?;
    out!(
        "{} rules, {} cases ok: {}",
        set.rules().len(),
        cases.cases.len(),
        paths.tilde(&paths.rules().display().to_string())
    );
    Ok(ExitCode::SUCCESS)
}

/// Exit code 1 if cases differ (TECHNIK.md 12.1).
pub fn test(paths: &Paths, config: &Config) -> Result<ExitCode, Error> {
    let set = load(paths, config)?;
    let cases = rules::cases(config)?;
    let failures = rules::check(&set, &cases.cases);
    if cases.skipped > 0 {
        log::info!("{} cases skipped for this config", cases.skipped);
    }
    if failures.is_empty() {
        out!("{} cases ok", cases.cases.len());
        return Ok(ExitCode::SUCCESS);
    }
    out!(
        "{} of {} cases differ{}",
        failures.len(),
        cases.cases.len(),
        rules::describe_failures(&failures)
    );
    Ok(ExitCode::from(1))
}

pub fn load(paths: &Paths, config: &Config) -> Result<RuleSet, Error> {
    let rules = store::read(&paths.rules(), config)?;
    Ok(RuleSet::new(rules, Markers::from_config(config)))
}
