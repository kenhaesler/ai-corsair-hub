//! V1→V2 config migration with atomic write and backup.
//!
//! ## Semantics
//!
//! - **All-or-nothing.** Every `(hub_serial, channel)` reference in the V1
//!   config must resolve to a device_id via the current [`DeviceRegistry`].
//!   If ANY reference fails, migration aborts with [`MigrationError::UnresolvedDevice`]
//!   and the file is not touched. The caller retries on the next boot.
//!
//! - **Idempotent.** If the file already has `schema_version >= 2`, the
//!   migration is a no-op.
//!
//! - **Backup before rewrite.** A `.bak.v1` sibling is created via `fs::copy`
//!   before the atomic write. If the user dislikes the result they can
//!   restore it manually.
//!
//! - **Atomic write.** Uses [`crate::atomic_write::write_atomic`] so a crash
//!   or power loss mid-write leaves either the old or the new content on
//!   disk, never a truncated prefix.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::atomic_write::write_atomic;
use crate::config::{
    v2::DeviceEntry, AppConfig, FanGroupConfig, RgbDeviceRef, SCHEMA_VERSION_V2,
};
use crate::identity::DeviceRegistry;

/// Errors that may occur during migration.
#[derive(Debug, thiserror::Error)]
pub enum MigrationError {
    /// A V1 `(hub_serial, channel)` reference has no device_id in the
    /// registry right now. Could be transient (device unplugged); caller
    /// should retry on next boot with a fresh registry.
    #[error("device at hub '{hub_serial}' channel {channel} not currently enumerated — cannot migrate")]
    UnresolvedDevice { hub_serial: String, channel: u8 },

    #[error("I/O error during migration: {0}")]
    Io(#[from] io::Error),

    #[error("TOML serialization failed: {0}")]
    TomlSer(#[from] toml::ser::Error),

    #[error("TOML parse failed: {0}")]
    TomlDe(#[from] toml::de::Error),
}

/// Outcome of [`try_migrate_file`].
#[derive(Debug)]
pub enum MigrationOutcome {
    /// File already on V2 schema — nothing done.
    AlreadyMigrated,
    /// Migration completed. `resolved_count` is the number of V1 device
    /// references rewritten to device_ids. The `.bak.v1` sibling was
    /// created with the pre-migration contents.
    Migrated { resolved_count: usize },
}

/// Compute the backup path for a config file.
///
/// Appends `.bak.v1` to the file name, preserving the full extension chain
/// (e.g. `config.toml` → `config.toml.bak.v1`). Using `with_extension`
/// directly would lose the `.toml` part.
fn backup_path_for(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "config".to_string());
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    parent.join(format!("{}.bak.v1", name))
}

/// Migrate a V1 `AppConfig` to V2 in memory. All-or-nothing: returns
/// [`MigrationError::UnresolvedDevice`] on the first `(hub_serial, channel)`
/// that the registry can't resolve, and leaves the input untouched.
///
/// The returned `AppConfig` has `schema_version = SCHEMA_VERSION_V2`, with
/// `fan_groups[].device_ids` and `rgb.zones[].devices[].device_id` populated
/// from the registry. V1 `device_overrides` become V2 `devices` entries
/// keyed by device_id. The V1-shaped fields (`channels`, `hub_serial`,
/// `device_overrides`) are cleared in the returned config so a subsequent
/// save produces a pure-V2 file.
pub fn migrate_v1_to_v2(
    v1: &AppConfig,
    registry: &DeviceRegistry,
) -> Result<AppConfig, MigrationError> {
    // Migrate fan groups. For each V1 group, resolve every (hub_serial,
    // channel) to a device_id. Empty hub_serial means the group was already
    // partially V2-shaped (shouldn't happen in pure V1 configs but we're
    // defensive about it).
    let mut new_fan_groups = Vec::with_capacity(v1.fan_groups.len());
    for group in &v1.fan_groups {
        let mut device_ids = Vec::with_capacity(group.channels.len());
        if let Some(hub_serial) = &group.hub_serial {
            for &channel in &group.channels {
                match registry.device_id_at(hub_serial, channel) {
                    Some(id) => {
                        device_ids.push(id.to_string());
                    }
                    None => {
                        return Err(MigrationError::UnresolvedDevice {
                            hub_serial: hub_serial.clone(),
                            channel,
                        });
                    }
                }
            }
        }
        // Preserve any existing device_ids (in case the config is
        // partially-migrated — shouldn't normally happen but keeps the
        // function total).
        for id in &group.device_ids {
            if !device_ids.contains(id) {
                device_ids.push(id.clone());
            }
        }

        new_fan_groups.push(FanGroupConfig {
            name: group.name.clone(),
            channels: Vec::new(),
            hub_serial: None,
            device_ids,
            mode: group.mode.clone(),
        });
    }

    // Migrate RGB zones. Same pattern — each (hub_serial, channel) in a zone
    // device ref must resolve.
    let mut new_rgb = v1.rgb.clone();
    for zone in &mut new_rgb.zones {
        let mut new_devs = Vec::with_capacity(zone.devices.len());
        for dev in &zone.devices {
            // If the ref already has a device_id, trust it (partial
            // migration tolerance) and skip the registry lookup. Otherwise
            // resolve (hub_serial, channel) → device_id.
            let device_id = if !dev.device_id.is_empty() {
                dev.device_id.clone()
            } else {
                match registry.device_id_at(&dev.hub_serial, dev.channel) {
                    Some(id) => id.to_string(),
                    None => {
                        return Err(MigrationError::UnresolvedDevice {
                            hub_serial: dev.hub_serial.clone(),
                            channel: dev.channel,
                        });
                    }
                }
            };
            new_devs.push(RgbDeviceRef {
                hub_serial: String::new(),
                channel: 0,
                device_id,
            });
        }
        zone.devices = new_devs;
    }

    // Migrate device_overrides (V1, keyed by (hub_serial, channel)) → V2
    // `devices` entries keyed by device_id. We merge with any V2 devices
    // already present in the input (shouldn't happen in pure V1 but
    // defensive).
    let mut new_devices: Vec<DeviceEntry> = v1.devices.clone();
    for ov in &v1.device_overrides {
        let device_id = match registry.device_id_at(&ov.hub_serial, ov.channel) {
            Some(id) => id.to_string(),
            None => {
                return Err(MigrationError::UnresolvedDevice {
                    hub_serial: ov.hub_serial.clone(),
                    channel: ov.channel,
                });
            }
        };
        // If an entry already exists for this device_id, update its
        // led_count; otherwise append. `name` is preserved when already
        // set.
        if let Some(existing) = new_devices.iter_mut().find(|d| d.device_id == device_id) {
            existing.led_count = Some(ov.led_count);
        } else {
            new_devices.push(DeviceEntry {
                device_id,
                name: None,
                led_count: Some(ov.led_count),
            });
        }
    }

    Ok(AppConfig {
        schema_version: SCHEMA_VERSION_V2,
        general: v1.general.clone(),
        fan_groups: new_fan_groups,
        rgb: new_rgb,
        device_overrides: Vec::new(),
        devices: new_devices,
    })
}

/// Read `path`, migrate if V1, write the V2 result atomically with a `.bak.v1`
/// backup.
///
/// Returns [`MigrationOutcome::AlreadyMigrated`] if the file is already V2
/// (no-op; no backup created, no write). On a missing/broken V1 reference,
/// returns [`MigrationError::UnresolvedDevice`] WITHOUT modifying anything
/// on disk — the caller retries on the next boot.
pub fn try_migrate_file(
    path: &Path,
    registry: &DeviceRegistry,
) -> Result<MigrationOutcome, MigrationError> {
    let contents = fs::read_to_string(path)?;
    let current: AppConfig = toml::from_str(&contents)?;

    if current.is_v2() {
        return Ok(MigrationOutcome::AlreadyMigrated);
    }

    let migrated = migrate_v1_to_v2(&current, registry)?;

    // Serialize BEFORE backup so a serialization failure doesn't leave a
    // useless backup file behind.
    let serialized = toml::to_string_pretty(&migrated)?;

    // Backup: straightforward copy of the raw V1 contents. Intentionally
    // not atomic — if this fails we abort and leave the original intact.
    let backup = backup_path_for(path);
    fs::copy(path, &backup)?;

    // Atomic write over the original.
    write_atomic(path, serialized.as_bytes())?;

    // resolved_count inside migrate_v1_to_v2 isn't returned across the
    // Result/Outcome boundary today; recompute it by walking the produced
    // V2 fan groups and rgb zones (cheap, and keeps the function
    // single-return).
    let resolved_count = migrated
        .fan_groups
        .iter()
        .map(|g| g.device_ids.len())
        .sum::<usize>()
        + migrated.rgb.zones.iter().map(|z| z.devices.len()).sum::<usize>()
        + current.device_overrides.len();

    Ok(MigrationOutcome::Migrated { resolved_count })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        CurvePoint, FanMode, GeneralConfig, LayerConfig, RgbConfig, RgbZoneConfig,
        TempSourceConfig,
    };
    use crate::identity::{DeviceEnumEntry, DeviceRegistry};
    use crate::config::DeviceOverride;
    use corsair_rgb::{BlendMode, EffectConfig};
    use tempfile::tempdir;

    fn registry_with_two_hubs() -> DeviceRegistry {
        DeviceRegistry::rebuild([
            DeviceEnumEntry {
                hub_serial: "HUB_A",
                device_id: "ID_A1",
                channel: 1,
                device_type_byte: 0x01,
                led_count: 34,
            },
            DeviceEnumEntry {
                hub_serial: "HUB_A",
                device_id: "ID_A2",
                channel: 2,
                device_type_byte: 0x01,
                led_count: 34,
            },
            DeviceEnumEntry {
                hub_serial: "HUB_B",
                device_id: "ID_B1",
                channel: 1,
                device_type_byte: 0x05,
                led_count: 21,
            },
        ])
    }

    fn v1_base() -> AppConfig {
        AppConfig {
            schema_version: 1,
            general: GeneralConfig {
                poll_interval_ms: 1000,
                log_level: "info".to_string(),
                lhm_exe_path: None,
            },
            fan_groups: Vec::new(),
            rgb: RgbConfig::default(),
            device_overrides: Vec::new(),
            devices: Vec::new(),
        }
    }

    #[test]
    fn migrates_v1_fan_groups_to_v2() {
        let mut v1 = v1_base();
        v1.fan_groups.push(FanGroupConfig {
            name: "top".into(),
            channels: vec![1, 2],
            hub_serial: Some("HUB_A".into()),
            device_ids: Vec::new(),
            mode: FanMode::Fixed { duty_percent: 50.0 },
        });

        let reg = registry_with_two_hubs();
        let v2 = migrate_v1_to_v2(&v1, &reg).expect("migration OK");

        assert_eq!(v2.schema_version, SCHEMA_VERSION_V2);
        assert_eq!(v2.fan_groups.len(), 1);
        assert_eq!(v2.fan_groups[0].device_ids, vec!["ID_A1", "ID_A2"]);
        // V1 fields cleared in the V2 output
        assert!(v2.fan_groups[0].channels.is_empty());
        assert!(v2.fan_groups[0].hub_serial.is_none());
    }

    #[test]
    fn migrates_rgb_zones_to_v2() {
        let mut v1 = v1_base();
        v1.rgb.zones.push(RgbZoneConfig {
            name: "All".into(),
            devices: vec![
                RgbDeviceRef {
                    hub_serial: "HUB_A".into(),
                    channel: 1,
                    device_id: String::new(),
                },
                RgbDeviceRef {
                    hub_serial: "HUB_B".into(),
                    channel: 1,
                    device_id: String::new(),
                },
            ],
            layers: vec![LayerConfig {
                effect: EffectConfig::Static {
                    color: corsair_rgb::Rgb::new(255, 0, 0),
                },
                blend_mode: BlendMode::Normal,
                opacity: 1.0,
                enabled: true,
            }],
            brightness: 100,
            flow: None,
        });

        let reg = registry_with_two_hubs();
        let v2 = migrate_v1_to_v2(&v1, &reg).expect("migration OK");

        assert_eq!(v2.rgb.zones.len(), 1);
        let devs = &v2.rgb.zones[0].devices;
        assert_eq!(devs.len(), 2);
        assert_eq!(devs[0].device_id, "ID_A1");
        assert_eq!(devs[1].device_id, "ID_B1");
        // V1 fields zeroed on the V2 output
        assert!(devs[0].hub_serial.is_empty());
        assert_eq!(devs[0].channel, 0);
    }

    #[test]
    fn migrates_device_overrides_to_v2() {
        let mut v1 = v1_base();
        v1.device_overrides.push(DeviceOverride {
            hub_serial: "HUB_A".into(),
            channel: 2,
            led_count: 30,
        });
        v1.device_overrides.push(DeviceOverride {
            hub_serial: "HUB_B".into(),
            channel: 1,
            led_count: 42,
        });

        let reg = registry_with_two_hubs();
        let v2 = migrate_v1_to_v2(&v1, &reg).expect("migration OK");

        assert!(v2.device_overrides.is_empty(), "V1 overrides cleared");
        assert_eq!(v2.devices.len(), 2);

        let a2 = v2.devices.iter().find(|d| d.device_id == "ID_A2").unwrap();
        assert_eq!(a2.led_count, Some(30));
        let b1 = v2.devices.iter().find(|d| d.device_id == "ID_B1").unwrap();
        assert_eq!(b1.led_count, Some(42));
    }

    #[test]
    fn aborts_when_device_unresolvable() {
        // Config references HUB_A channel 99, which is NOT in the registry.
        let mut v1 = v1_base();
        v1.fan_groups.push(FanGroupConfig {
            name: "ghost".into(),
            channels: vec![99],
            hub_serial: Some("HUB_A".into()),
            device_ids: Vec::new(),
            mode: FanMode::Fixed { duty_percent: 50.0 },
        });

        let reg = registry_with_two_hubs();
        let err = migrate_v1_to_v2(&v1, &reg).expect_err("must fail");
        match err {
            MigrationError::UnresolvedDevice { hub_serial, channel } => {
                assert_eq!(hub_serial, "HUB_A");
                assert_eq!(channel, 99);
            }
            other => panic!("unexpected error: {:?}", other),
        }
    }

    /// The file-level migration must NOT modify the target file if any
    /// reference is unresolvable. Verifies pre-migration byte contents equal
    /// post-attempt byte contents.
    #[test]
    fn aborts_file_migration_when_device_unresolvable() {
        let dir = tempdir().unwrap();
        let cfg_path = dir.path().join("config.toml");

        let mut v1 = v1_base();
        v1.fan_groups.push(FanGroupConfig {
            name: "ghost".into(),
            channels: vec![99], // not in registry
            hub_serial: Some("HUB_A".into()),
            device_ids: Vec::new(),
            mode: FanMode::Fixed { duty_percent: 50.0 },
        });
        let original = toml::to_string_pretty(&v1).unwrap();
        fs::write(&cfg_path, &original).unwrap();

        let reg = registry_with_two_hubs();
        let res = try_migrate_file(&cfg_path, &reg);
        assert!(matches!(res, Err(MigrationError::UnresolvedDevice { .. })));

        // File unchanged.
        let after = fs::read_to_string(&cfg_path).unwrap();
        assert_eq!(after, original, "file must be untouched when migration aborts");
        // No backup file created on abort.
        let backup = backup_path_for(&cfg_path);
        assert!(!backup.exists(), "no backup on abort");
    }

    #[test]
    fn creates_bak_file_and_uses_atomic_write() {
        let dir = tempdir().unwrap();
        let cfg_path = dir.path().join("config.toml");

        // Construct a V1 config with one fan group and one override, all
        // resolvable.
        let mut v1 = v1_base();
        v1.fan_groups.push(FanGroupConfig {
            name: "top".into(),
            channels: vec![1, 2],
            hub_serial: Some("HUB_A".into()),
            device_ids: Vec::new(),
            mode: FanMode::Curve {
                points: vec![
                    CurvePoint { temp: 40.0, duty: 30.0 },
                    CurvePoint { temp: 70.0, duty: 100.0 },
                ],
                hysteresis: 2.0,
                ramp_rate: 5.0,
                temp_source: TempSourceConfig {
                    sensors: vec!["cpu".into()],
                    weights: vec![1.0],
                },
            },
        });
        v1.device_overrides.push(DeviceOverride {
            hub_serial: "HUB_B".into(),
            channel: 1,
            led_count: 42,
        });

        let original = toml::to_string_pretty(&v1).unwrap();
        fs::write(&cfg_path, &original).unwrap();

        let reg = registry_with_two_hubs();
        let outcome = try_migrate_file(&cfg_path, &reg).expect("migration OK");
        match outcome {
            MigrationOutcome::Migrated { resolved_count } => {
                // 2 channels + 1 override = 3 references resolved
                assert_eq!(resolved_count, 3);
            }
            other => panic!("expected Migrated, got {:?}", other),
        }

        // Backup exists, parses back to V1 shape (schema_version = 1).
        let backup = backup_path_for(&cfg_path);
        assert!(backup.exists(), ".bak.v1 exists");
        let backup_contents = fs::read_to_string(&backup).unwrap();
        assert_eq!(backup_contents, original, "backup is byte-identical to pre-migration");
        let reparsed_backup: AppConfig = toml::from_str(&backup_contents).unwrap();
        assert_eq!(reparsed_backup.schema_version, 1);

        // New file parses as V2.
        let new_contents = fs::read_to_string(&cfg_path).unwrap();
        let v2: AppConfig = toml::from_str(&new_contents).unwrap();
        assert_eq!(v2.schema_version, SCHEMA_VERSION_V2);
        assert_eq!(v2.fan_groups[0].device_ids, vec!["ID_A1", "ID_A2"]);
        assert!(v2.device_overrides.is_empty());
        assert_eq!(v2.devices.len(), 1);
        assert_eq!(v2.devices[0].device_id, "ID_B1");
        assert_eq!(v2.devices[0].led_count, Some(42));
    }

    /// Second migration call on an already-V2 file is a no-op.
    #[test]
    fn already_migrated_is_noop() {
        let dir = tempdir().unwrap();
        let cfg_path = dir.path().join("config.toml");

        let mut v2 = v1_base();
        v2.schema_version = SCHEMA_VERSION_V2;
        let serialized = toml::to_string_pretty(&v2).unwrap();
        fs::write(&cfg_path, &serialized).unwrap();

        let reg = registry_with_two_hubs();
        let outcome = try_migrate_file(&cfg_path, &reg).expect("no-op OK");
        assert!(matches!(outcome, MigrationOutcome::AlreadyMigrated));

        // No backup created, file unchanged.
        let backup = backup_path_for(&cfg_path);
        assert!(!backup.exists(), "no backup on already-V2");
        let after = fs::read_to_string(&cfg_path).unwrap();
        assert_eq!(after, serialized);
    }
}
