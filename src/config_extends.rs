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
//!
//! It also reads each file the way the loader does, in the loader's order: ask
//! the parent-directory capability for the candidate's metadata without
//! resolving the link, then canonicalize, then read through that capability.
//! The order is load-bearing. Canonicalizing first would read a symlink's
//! target even when the target escapes the candidate's own directory, which the
//! loader refuses unread; and opening the candidate before statting it would
//! block forever on a FIFO the loader rejects. Either way the pre-scan would be
//! looking at bytes the loader never would, so it repeats the sequence rather
//! than approximating it.

use std::{
    ffi::OsString,
    io,
    path::{Path, PathBuf},
};

use cap_std::{ambient_authority, fs::Dir};
use ortho_config::{ConfigDiscovery, EnvSource, file::canonicalise, toml};

use super::{ConfigError, schema::SELECTOR_VARIABLE};

/// The key whose presence makes a file refuse to load.
const EXTENDS_KEY: &str = "extends";

/// Rejects any configuration file the policy would load that declares `extends`.
///
/// The check runs before resolution, so a refused file stops the run rather
/// than being loaded and reported afterwards. A file the loader will not read —
/// absent, outside the directory the candidate names, not a regular file — and
/// one that is not valid TOML are all left alone: the loader meets them next
/// and reports them with the diagnostic this tool already documents, so the
/// pre-scan changes nothing about how those failures look.
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
/// The loader does two things before it parses anything, and this repeats both
/// in its order. It first asks its parent-directory capability for the
/// candidate's metadata *without* resolving the link, and only then
/// canonicalizes and reads what that resolves to. A candidate the loader would
/// refuse — one outside the directory it names, or one that is not a regular
/// file — is therefore left alone here as well, and the loader reports it in
/// the words this tool already documents.
///
/// Getting that order wrong is not cosmetic. Canonicalizing first would read a
/// symlink's target even when the target lies outside the candidate's
/// directory, which the loader never does; and opening a candidate rather than
/// statting it would block forever on a FIFO that the loader rejects unread.
///
/// A type other than string still counts: `extends = 5` is refused here rather
/// than left for the loader to reject, because the key is the thing this module
/// exists to keep out of the loader's reach.
fn declares_extends(path: &Path) -> bool {
    if !is_loadable_candidate(path) {
        return false;
    }
    let Ok(resolved) = canonicalise(path) else {
        return false;
    };
    let Ok(text) = read_file(&resolved) else {
        return false;
    };
    let Ok(document) = toml::from_str::<toml::Value>(&text) else {
        return false;
    };

    document.get(EXTENDS_KEY).is_some()
}

/// Whether the loader would go on to read `path` at all.
///
/// This mirrors `file_exists_and_is_regular` exactly: the same
/// `open_parent_dir_and_name`, the same metadata query on the unresolved name.
/// The capability is what enforces the boundary — a symlink is followed only
/// when its target stays inside the directory — so the pre-scan follows one in
/// precisely the cases the loader will.
fn is_loadable_candidate(path: &Path) -> bool {
    let Some((directory, file_name)) = parent_capability(path) else {
        return false;
    };

    matches!(directory.metadata(file_name), Ok(metadata) if metadata.is_file())
}

/// Opens `path`'s parent as a capability, with the bare name inside it.
///
/// The parent is opened as a capability rather than the file being opened by
/// absolute path, so the read is confined to the directory the path names even
/// when a component of that path is a symlink to somewhere else. Returning the
/// pair lets the metadata query and the read share one resolution step, so
/// they cannot disagree about which entry is meant.
fn parent_capability(path: &Path) -> Option<(Dir, OsString)> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let file_name = path.file_name()?.to_owned();
    let directory = Dir::open_ambient_dir(parent, ambient_authority()).ok()?;

    Some((directory, file_name))
}

/// Reads `path` through its parent directory's capability.
fn read_file(path: &Path) -> io::Result<String> {
    let (directory, file_name) = parent_capability(path)
        .ok_or_else(|| io::Error::other("configuration path has no file name"))?;

    directory.read_to_string(file_name)
}
