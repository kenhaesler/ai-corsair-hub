//! Startup config loading with recovery.
//!
//! Policy (see `docs/config-startup-recovery.md`):
//! * Primary `config.toml` missing => first install: legacy `cwd/config.toml`
//!   if present, else defaults (no issue).
//! * Primary present but unreadable / unparsable / invalid => the original is
//!   NEVER modified. Its bytes are copied to a sibling
//!   `config.invalid-<UTC stamp>.toml` (never overwriting an existing file),
//!   then `config.last-good.toml` is used if it loads and validates, else
//!   defaults. The legacy file is not consulted in this case.
//! * Primary loads and validates => its exact bytes are mirrored to
//!   `config.last-good.toml`.

use std::fs::OpenOptions;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::Serialize;
use tracing::{error, info, warn};

use corsair_common::atomic_write::write_atomic;
use corsair_common::config::AppConfig;
use corsair_fancontrol::control_loop;

const LAST_GOOD_FILE: &str = "config.last-good.toml";

/// Reported to the frontend via `get_startup_config_issue` when the on-disk
/// config could not be used at startup.
#[derive(Debug, Clone, Serialize)]
pub struct StartupConfigIssue {
    /// The file that failed to load.
    pub config_path: String,
    /// Where a byte-identical copy of it was preserved, if that succeeded.
    pub preserved_path: Option<String>,
    pub preserve_error: Option<String>,
    /// Why it failed (read, parse or validation error).
    pub error: String,
    /// What is running instead: `"last_good"` or `"defaults"`.
    pub loaded: String,
}

pub fn load_startup_config(
    path: &Path,
    legacy_path: &Path,
) -> (AppConfig, Option<StartupConfigIssue>) {
    load_startup_config_at(path, legacy_path, SystemTime::now())
}

fn parse_and_validate(bytes: &[u8]) -> anyhow::Result<AppConfig> {
    let text = std::str::from_utf8(bytes)
        .map_err(|e| anyhow::anyhow!("Config is not valid UTF-8: {e}"))?;
    let config: AppConfig =
        toml::from_str(text).map_err(|e| anyhow::anyhow!("Invalid TOML config: {e}"))?;
    control_loop::validate_config(&config)?;
    Ok(config)
}

fn load_startup_config_at(
    path: &Path,
    legacy_path: &Path,
    now: SystemTime,
) -> (AppConfig, Option<StartupConfigIssue>) {
    let last_good = path.with_file_name(LAST_GOOD_FILE);
    match std::fs::read(path) {
        Ok(bytes) => match parse_and_validate(&bytes) {
            Ok(config) => {
                info!("Loaded config from {}", path.display());
                record_last_good(path, &bytes);
                (config, None)
            }
            Err(e) => recover(path, Ok(bytes), format!("{e:#}"), &last_good, now),
        },
        Err(e) if e.kind() == ErrorKind::NotFound => {
            load_legacy_or_default(legacy_path, &last_good, now)
        }
        Err(e) => recover(
            path,
            Err(e.to_string()),
            format!("Cannot read config: {e}"),
            &last_good,
            now,
        ),
    }
}

/// First install (no primary file): try the legacy cwd location, else defaults.
fn load_legacy_or_default(
    legacy_path: &Path,
    last_good: &Path,
    now: SystemTime,
) -> (AppConfig, Option<StartupConfigIssue>) {
    match std::fs::read(legacy_path) {
        Ok(bytes) => match parse_and_validate(&bytes) {
            Ok(config) => {
                info!("Loaded legacy config from {}", legacy_path.display());
                (config, None)
            }
            Err(e) => recover(legacy_path, Ok(bytes), format!("{e:#}"), last_good, now),
        },
        Err(e) if e.kind() == ErrorKind::NotFound => {
            info!("Using default config");
            (AppConfig::default(), None)
        }
        Err(e) => recover(
            legacy_path,
            Err(e.to_string()),
            format!("Cannot read config: {e}"),
            last_good,
            now,
        ),
    }
}

/// Preserve the failing file, then pick last-good or defaults.
/// `last_good` always lives next to the primary config (`bad` may be the
/// legacy file).
fn recover(
    bad: &Path,
    bytes: Result<Vec<u8>, String>,
    err: String,
    last_good: &Path,
    now: SystemTime,
) -> (AppConfig, Option<StartupConfigIssue>) {
    let (preserved, preserve_error) = match bytes {
        Ok(b) => match preserve_invalid(bad, &b, now) {
            Ok(p) => (Some(p), None),
            Err(e) => (None, Some(e)),
        },
        Err(e) => (None, Some(format!("original could not be read: {e}"))),
    };
    let (config, loaded) = load_last_good_or_default(last_good);
    error!(
        config_path = %bad.display(),
        preserved_path = ?preserved.as_ref().map(|p| p.display().to_string()),
        preserve_error = ?preserve_error,
        loaded,
        "Config is unusable: {err}. The original file was left untouched; running on {loaded}"
    );
    let issue = StartupConfigIssue {
        config_path: bad.display().to_string(),
        preserved_path: preserved.map(|p| p.display().to_string()),
        preserve_error,
        error: err,
        loaded: loaded.to_string(),
    };
    (config, Some(issue))
}

fn load_last_good_or_default(last_good: &Path) -> (AppConfig, &'static str) {
    if last_good.exists() {
        match control_loop::load_config(last_good) {
            Ok(config) => return (config, "last_good"),
            Err(e) => warn!("Last-known-good config unusable ({}): {e:#}", last_good.display()),
        }
    }
    (AppConfig::default(), "defaults")
}

/// Copy `bytes` to `<dir>/config.invalid-<stamp>[-N].toml`, never overwriting.
fn preserve_invalid(bad: &Path, bytes: &[u8], now: SystemTime) -> Result<PathBuf, String> {
    let stamp = crate::diagnostics::utc_stamps(now)
        .1
        .replace(['-', ':'], "");
    for n in 0..1000u32 {
        let name = if n == 0 {
            format!("config.invalid-{stamp}.toml")
        } else {
            format!("config.invalid-{stamp}-{n}.toml")
        };
        let target = bad.with_file_name(name);
        match OpenOptions::new().write(true).create_new(true).open(&target) {
            Ok(mut f) => {
                f.write_all(bytes)
                    .and_then(|()| f.sync_all())
                    .map_err(|e| format!("writing {}: {e}", target.display()))?;
                return Ok(target);
            }
            Err(e) if e.kind() == ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("creating {}: {e}", target.display())),
        }
    }
    Err("no free config.invalid-* file name".to_string())
}

/// Mirror the bytes of a config that just loaded and validated.
fn record_last_good(primary: &Path, bytes: &[u8]) {
    let last_good = primary.with_file_name(LAST_GOOD_FILE);
    if std::fs::read(&last_good).is_ok_and(|existing| existing == bytes) {
        return;
    }
    if let Err(e) = write_atomic(&last_good, bytes) {
        warn!("Could not update {}: {e}", last_good.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};

    const VALID: &str = "[general]\npoll_interval_ms = 500\nlog_level = \"info\"\n";
    const VALID_OTHER: &str = "[general]\npoll_interval_ms = 700\nlog_level = \"info\"\n";
    // Parses, but fails validation (min_duty > max_duty).
    const PARSES_INVALID: &str = r#"
[general]
poll_interval_ms = 1000
log_level = "info"

[[fan_groups]]
name = "g"
channels = [1]
hub_serial = "H"

[fan_groups.mode]
type = "pid"
target_temp = 60.0
kp = 1.0
ki = 0.1
kd = 0.0
min_duty = 80.0
max_duty = 40.0

[fan_groups.mode.temp_source]
sensors = ["cpu"]
weights = [1.0]
"#;
    const BROKEN_TOML: &str = "[general\npoll_interval_ms = ";

    fn t(secs: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(secs)
    }

    /// 1_700_000_000 s = 2023-11-14 22:13:20 UTC.
    const STAMP: &str = "20231114T221320Z";

    fn setup() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let primary = dir.path().join("app").join("config.toml");
        std::fs::create_dir_all(primary.parent().unwrap()).unwrap();
        let legacy = dir.path().join("cwd").join("config.toml");
        std::fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        (dir, primary, legacy)
    }

    fn files(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }

    #[test]
    fn backup_stamp_is_compact_utc() {
        let (_d, primary, legacy) = setup();
        std::fs::write(&primary, BROKEN_TOML).unwrap();
        let (_, issue) = load_startup_config_at(&primary, &legacy, t(1_700_000_000));
        let p = issue.unwrap().preserved_path.unwrap();
        assert!(p.ends_with(&format!("config.invalid-{STAMP}.toml")), "{p}");
        // Leap day and epoch, via the shared formatter.
        let f = |s| crate::diagnostics::utc_stamps(t(s)).1.replace(['-', ':'], "");
        assert_eq!(f(0), "19700101T000000Z");
        assert_eq!(f(951_782_400), "20000229T000000Z");
    }

    #[test]
    fn invalid_toml_is_preserved_and_defaults_used() {
        let (_d, primary, legacy) = setup();
        std::fs::write(&primary, BROKEN_TOML).unwrap();
        // A valid legacy file must NOT be used when the primary exists.
        std::fs::write(&legacy, VALID).unwrap();

        let (cfg, issue) = load_startup_config_at(&primary, &legacy, t(1_700_000_000));
        let issue = issue.expect("issue reported");
        assert_eq!(cfg.general.poll_interval_ms, 1000, "defaults");
        assert_eq!(issue.loaded, "defaults");
        assert_eq!(issue.config_path, primary.display().to_string());
        assert!(issue.error.contains("Invalid TOML"), "{}", issue.error);
        assert!(issue.preserve_error.is_none());
        let preserved = PathBuf::from(issue.preserved_path.unwrap());
        assert_eq!(std::fs::read(&preserved).unwrap(), BROKEN_TOML.as_bytes());
        assert_eq!(std::fs::read(&primary).unwrap(), BROKEN_TOML.as_bytes());
        assert_eq!(
            files(primary.parent().unwrap()),
            vec!["config.invalid-20231114T221320Z.toml", "config.toml"]
        );
    }

    #[test]
    fn parses_but_fails_validation_is_preserved() {
        let (_d, primary, legacy) = setup();
        std::fs::write(&primary, PARSES_INVALID).unwrap();
        let (cfg, issue) = load_startup_config_at(&primary, &legacy, t(1_700_000_000));
        let issue = issue.unwrap();
        assert!(cfg.fan_groups.is_empty());
        assert_eq!(issue.loaded, "defaults");
        assert!(issue.error.contains("min_duty"), "{}", issue.error);
        let preserved = PathBuf::from(issue.preserved_path.unwrap());
        assert_eq!(std::fs::read(preserved).unwrap(), PARSES_INVALID.as_bytes());
        assert_eq!(std::fs::read(&primary).unwrap(), PARSES_INVALID.as_bytes());
    }

    #[test]
    fn existing_backup_is_never_overwritten() {
        let (_d, primary, legacy) = setup();
        std::fs::write(&primary, BROKEN_TOML).unwrap();
        let old = primary.with_file_name(format!("config.invalid-{STAMP}.toml"));
        std::fs::write(&old, "OLDER BACKUP").unwrap();

        let (_, issue) = load_startup_config_at(&primary, &legacy, t(1_700_000_000));
        let new = PathBuf::from(issue.unwrap().preserved_path.unwrap());
        assert_ne!(new, old);
        assert!(new.to_string_lossy().ends_with(&format!("config.invalid-{STAMP}-1.toml")));
        assert_eq!(std::fs::read(&old).unwrap(), b"OLDER BACKUP");
        assert_eq!(std::fs::read(&new).unwrap(), BROKEN_TOML.as_bytes());

        // A third collision goes to -2.
        let (_, issue) = load_startup_config_at(&primary, &legacy, t(1_700_000_000));
        let third = issue.unwrap().preserved_path.unwrap();
        assert!(third.ends_with(&format!("config.invalid-{STAMP}-2.toml")), "{third}");
    }

    #[test]
    fn last_good_is_used_when_valid() {
        let (_d, primary, legacy) = setup();
        std::fs::write(&primary, BROKEN_TOML).unwrap();
        std::fs::write(primary.with_file_name(LAST_GOOD_FILE), VALID_OTHER).unwrap();
        let (cfg, issue) = load_startup_config_at(&primary, &legacy, t(1_700_000_000));
        assert_eq!(cfg.general.poll_interval_ms, 700);
        assert_eq!(issue.unwrap().loaded, "last_good");
        assert_eq!(std::fs::read(&primary).unwrap(), BROKEN_TOML.as_bytes());
        assert_eq!(
            std::fs::read(primary.with_file_name(LAST_GOOD_FILE)).unwrap(),
            VALID_OTHER.as_bytes()
        );
    }

    #[test]
    fn invalid_last_good_falls_back_to_defaults() {
        let (_d, primary, legacy) = setup();
        std::fs::write(&primary, BROKEN_TOML).unwrap();
        std::fs::write(primary.with_file_name(LAST_GOOD_FILE), PARSES_INVALID).unwrap();
        let (cfg, issue) = load_startup_config_at(&primary, &legacy, t(1_700_000_000));
        assert_eq!(cfg.general.poll_interval_ms, 1000);
        assert_eq!(issue.unwrap().loaded, "defaults");
    }

    #[test]
    fn missing_primary_and_legacy_gives_defaults_without_side_effects() {
        let (_d, primary, legacy) = setup();
        let (cfg, issue) = load_startup_config_at(&primary, &legacy, t(1_700_000_000));
        assert!(issue.is_none());
        assert_eq!(cfg.general.poll_interval_ms, 1000);
        assert!(files(primary.parent().unwrap()).is_empty());
        assert!(files(legacy.parent().unwrap()).is_empty());
    }

    #[test]
    fn valid_legacy_is_used_when_primary_missing() {
        let (_d, primary, legacy) = setup();
        std::fs::write(&legacy, VALID).unwrap();
        let (cfg, issue) = load_startup_config_at(&primary, &legacy, t(1_700_000_000));
        assert!(issue.is_none());
        assert_eq!(cfg.general.poll_interval_ms, 500);
        assert!(files(primary.parent().unwrap()).is_empty());
    }

    #[test]
    fn invalid_legacy_is_preserved_and_reported() {
        let (_d, primary, legacy) = setup();
        std::fs::write(&legacy, PARSES_INVALID).unwrap();
        let (cfg, issue) = load_startup_config_at(&primary, &legacy, t(1_700_000_000));
        let issue = issue.unwrap();
        assert_eq!(cfg.general.poll_interval_ms, 1000);
        assert_eq!(issue.loaded, "defaults");
        assert_eq!(issue.config_path, legacy.display().to_string());
        let preserved = PathBuf::from(issue.preserved_path.unwrap());
        assert_eq!(preserved.parent(), legacy.parent());
        assert_eq!(std::fs::read(preserved).unwrap(), PARSES_INVALID.as_bytes());
        assert_eq!(std::fs::read(&legacy).unwrap(), PARSES_INVALID.as_bytes());
        assert!(files(primary.parent().unwrap()).is_empty());
    }

    #[test]
    fn invalid_legacy_uses_last_good_next_to_primary() {
        let (_d, primary, legacy) = setup();
        std::fs::write(&legacy, BROKEN_TOML).unwrap();
        std::fs::write(primary.with_file_name(LAST_GOOD_FILE), VALID_OTHER).unwrap();
        let (cfg, issue) = load_startup_config_at(&primary, &legacy, t(1_700_000_000));
        assert_eq!(cfg.general.poll_interval_ms, 700);
        assert_eq!(issue.unwrap().loaded, "last_good");
    }

    #[test]
    fn valid_primary_records_last_good_byte_identical() {
        let (_d, primary, legacy) = setup();
        // Odd formatting/comment must survive: the copy is bytes, not a re-serialization.
        let src = format!("# my comment\r\n{VALID}");
        std::fs::write(&primary, &src).unwrap();
        let last_good = primary.with_file_name(LAST_GOOD_FILE);

        let (cfg, issue) = load_startup_config_at(&primary, &legacy, t(1_700_000_000));
        assert!(issue.is_none());
        assert_eq!(cfg.general.poll_interval_ms, 500);
        assert_eq!(std::fs::read(&last_good).unwrap(), src.as_bytes());
        assert_eq!(files(primary.parent().unwrap()), vec!["config.last-good.toml", "config.toml"]);

        // Second start: intact, no extra files.
        let (_, issue) = load_startup_config_at(&primary, &legacy, t(1_700_000_100));
        assert!(issue.is_none());
        assert_eq!(std::fs::read(&last_good).unwrap(), src.as_bytes());
        assert_eq!(files(primary.parent().unwrap()), vec!["config.last-good.toml", "config.toml"]);
    }

    #[test]
    fn invalid_primary_does_not_touch_last_good() {
        let (_d, primary, legacy) = setup();
        std::fs::write(&primary, VALID).unwrap();
        load_startup_config_at(&primary, &legacy, t(1));
        std::fs::write(&primary, BROKEN_TOML).unwrap();
        let (cfg, _) = load_startup_config_at(&primary, &legacy, t(1_700_000_000));
        assert_eq!(cfg.general.poll_interval_ms, 500);
        assert_eq!(
            std::fs::read(primary.with_file_name(LAST_GOOD_FILE)).unwrap(),
            VALID.as_bytes()
        );
    }
}
