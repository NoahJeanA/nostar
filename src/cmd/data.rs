//! `nostar data update`

use crate::Error;
use crate::config::Config;
use crate::fsutil::Lock;
use crate::paths::Paths;
use crate::source;

pub fn update(paths: &Paths, config: &Config) -> Result<(), Error> {
    let _lock = Lock::acquire(&paths.lock())?;
    source::update(paths, &config.source)?;
    Ok(())
}
