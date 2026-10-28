//! `nostar data update`

use crate::Error;
use crate::config::Config;
use crate::fsutil::Lock;
use crate::paths::Paths;
use crate::source;

pub fn update(paths: &Paths, config: &Config) -> Result<(), Error> {
    let _lock = Lock::acquire(&paths.lock())?;
    let path = source::update(paths, &config.source)?;
    let short: String = config.source.commit.chars().take(7).collect();
    out!(
        "dictionary entgendert@{short}, sha256 ok: {}",
        paths.tilde(&path.display().to_string())
    );
    Ok(())
}
