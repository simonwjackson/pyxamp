//! Source configuration: one non-secret TOML file, secrets read from files.
//!
//! Path: `$PYXAMP_SOURCES`, else `$XDG_CONFIG_HOME/pyxamp/sources.toml`, else
//! `~/.config/pyxamp/sources.toml`. A missing file means no sources. A missing
//! section means that source is absent. There is no sign-in screen.
//!
//! ```toml
//! [pyxis]
//! url = "https://pyxis.example.ts.net"
//! token_file = "/run/secrets/pyxamp/pyxis-token"
//! device_name = "Myoko"          # optional; the name this host shows in Play on
//!
//! [ossicle]
//! url = "https://ossicle.example.ts.net"
//! token_file = "/run/secrets/pyxamp/ossicle-token"   # optional when Ossicle needs none
//!
//! [jellyfin]
//! url = "https://jellyfin.example.ts.net"
//! api_key_file = "/run/secrets/pyxamp/jellyfin-api-key"
//! user = "simon"
//! ```
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Sources {
    pub pyxis: Option<PyxisConfig>,
    pub ossicle: Option<OssicleConfig>,
    pub jellyfin: Option<JellyfinConfig>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PyxisConfig {
    pub url: String,
    pub token_file: PathBuf,
    pub device_name: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct OssicleConfig {
    pub url: String,
    pub token_file: Option<PathBuf>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct JellyfinConfig {
    pub url: String,
    pub api_key_file: PathBuf,
    pub user: String,
}

pub fn default_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("PYXAMP_SOURCES") {
        return Some(path.into());
    }
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join("pyxamp/sources.toml"))
}

/// A missing file is no sources. The daemon logs a malformed file and starts with no sources.
pub fn load(path: Option<&Path>) -> anyhow::Result<Sources> {
    let Some(path) = path else {
        return Ok(Sources::default());
    };
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(toml::from_str(&text)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Sources::default()),
        Err(error) => Err(error.into()),
    }
}

/// Read a secret at use time; trailing whitespace is trimmed. Never log the value.
pub fn read_secret(path: &Path) -> anyhow::Result<String> {
    let value = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("cannot read secret {}: {e}", path.display()))?;
    let value = value.trim_end().to_owned();
    anyhow::ensure!(!value.is_empty(), "secret {} is empty", path.display());
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_and_sections_mean_absent_sources() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            load(Some(&dir.path().join("none.toml"))).unwrap(),
            Sources::default()
        );
        let path = dir.path().join("sources.toml");
        std::fs::write(
            &path,
            "[jellyfin]\nurl = \"https://j\"\napi_key_file = \"/k\"\nuser = \"simon\"\n",
        )
        .unwrap();
        let sources = load(Some(&path)).unwrap();
        assert!(sources.pyxis.is_none() && sources.ossicle.is_none());
        assert_eq!(sources.jellyfin.unwrap().user, "simon");
        std::fs::write(&path, "[pyxis]\nurl = \"x\"\ntoken = \"inline\"\n").unwrap();
        assert!(load(Some(&path)).is_err(), "inline secrets are refused");
    }
}
