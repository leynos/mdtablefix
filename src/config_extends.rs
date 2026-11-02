//! Refusing the inherited `extends` key before any file is loaded.
//!
//! `ortho_config` 0.10.0 expands an `extends` chain itself: a file that names a
//! parent causes the parent to be read and parsed first. Neither the crate's
//! features nor its builder offers a way to turn that off, and the selector
//! machinery that could load files by hand is private. A configuration file in a
//! working directory could therefore name `../credentials`, and the parent would
//! be read.
//!
//! Reading is not the danger by itself; printing is. The loader validates TOML
//! with a parser whose error quotes the offending source line, and the
//! composition root prints configuration errors to standard error. A malformed
//! parent — a credentials file is malformed TOML almost by definition — would
//! therefore put one of its lines into the log of any run started in that
//! directory.
//!
//! This module closes the route before the loader runs. It reads the same files
//! the policy is about to read and refuses any that declares a top-level
//! `extends`, so no parent is ever opened. Refusing is a deliberate narrowing
//! rather than a loss: inheritance has never been documented for this tool, its
//! settings are a flat list of switches, and a file that wants to share them can
//! state them directly.
//!
//! The pre-scan has to see exactly the files the policy will load, or it either
//! misses one or rejects a file that would have been ignored. It therefore
//! applies the policy's own order: a selected path, when the selector names one,
//! suppresses automatic discovery entirely, and only otherwise are the
//! discovered candidates consulted. [`super::merged_config`] passes the same
//! discovery and the same environment source to both, so the two cannot disagree
//! about *where* to look.

use std::{
    io,
    path::{Path, PathBuf},
};

use cap_std::{ambient_authority, fs::Dir};
use ortho_config::{ConfigDiscovery, EnvSource, toml};

use super::{ConfigError, schema::SELECTOR_VARIABLE};

/// The key whose presence makes a file refuse to load.
const EXTENDS_KEY: &str = "extends";

/// Rejects any configuration file the policy would load that declares `extends`.
///
/// The check runs before resolution, so a refused file stops the run rather
/// than being loaded and reported afterwards. A file that cannot be read or is
/// not valid TOML is left alone: the loader meets it next and reports it with
/// the diagnostic this tool already documents, so the pre-scan changes nothing
/// about how those failures look.
pub(super) fn refuse(
    discovery: &ConfigDiscovery,
    lookup: &dyn EnvSource,
) -> Result<(), ConfigError> {
    for path in inputs(discovery, lookup) {
        if declares_extends(&path) {
            return Err(ConfigError::UnsupportedExtends { path });
        }
    }

    Ok(())
}

/// The files the policy will attempt to load, in the order it attempts them.
///
/// A selector that names a path is exclusive: `ConfigFilePolicy` loads that file
/// and nothing else, whatever else exists. Mirroring that here is what keeps a
/// broken user file from failing a run whose selector names a good one.
fn inputs(discovery: &ConfigDiscovery, lookup: &dyn EnvSource) -> Vec<PathBuf> {
    selected_path(lookup).map_or_else(|| discovery.candidates(), |path| vec![path])
}

/// The path `MDTABLEFIX_CONFIG_PATH` names, when it names one.
///
/// This repeats `ConfigPathSelector`'s rule — set and non-empty — because the
/// selector resolves internally and reports its answer only by loading the file.
/// The two must agree exactly; a test pins the exclusive case so a future
/// divergence is caught rather than silently widening the pre-scan.
fn selected_path(lookup: &dyn EnvSource) -> Option<PathBuf> {
    lookup
        .get(SELECTOR_VARIABLE)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// Whether `path` is valid TOML declaring a top-level `extends` key.
///
/// A type other than string still counts: `extends = 5` is refused here rather
/// than left for the loader to reject, because the key is the thing this module
/// exists to keep out of the loader's reach.
fn declares_extends(path: &Path) -> bool {
    let Ok(text) = read_file(path) else {
        return false;
    };
    let Ok(document) = toml::from_str::<toml::Value>(&text) else {
        return false;
    };

    document.get(EXTENDS_KEY).is_some()
}

/// Reads a file relative to its own parent directory.
///
/// The parent is opened as a capability rather than the file being opened by
/// absolute path, so the read is confined to the directory the path names even
/// when a component of that path is a symlink to somewhere else. This is the
/// same shape `src/driver/run.rs` uses for the files it formats, and it keeps
/// `std::fs` out of the binary's configuration code.
fn read_file(path: &Path) -> io::Result<String> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let file_name = path
        .file_name()
        .ok_or_else(|| io::Error::other("configuration path has no file name"))?;
    let directory = Dir::open_ambient_dir(parent, ambient_authority())?;

    directory.read_to_string(file_name)
}
