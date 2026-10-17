# Config startup recovery and early validation (G06)

No TOML schema, serde attribute or field name changed. Configs that were valid
before load identically.

## Startup policy

Implemented in `apps/gui/src/startup_config.rs` (`load_startup_config`).

| Situation | Result |
|---|---|
| `%APPDATA%\corsair-hub\config.toml` missing, legacy `cwd/config.toml` valid | legacy file used, no issue |
| primary and legacy both missing | defaults, no issue, no files created |
| primary exists, loads and validates | used; bytes mirrored to `config.last-good.toml` |
| primary exists but cannot be read / parsed / validated | original left untouched; bytes copied to `config.invalid-<UTC yyyymmddThhmmssZ>.toml`; then `config.last-good.toml` if it loads and validates, else defaults; issue reported |
| primary missing, legacy exists but invalid | legacy preserved the same way (next to the legacy file); last-good next to the primary, else defaults; issue reported |

Selection rule: when the primary exists but is unusable, the legacy file is
never consulted. The order is primary, then last-good, then defaults.

Backups are created with `create_new`, so an existing backup is never
overwritten; a name collision appends `-1`, `-2`, ... If the original cannot be
read or copied, `preserve_error` says so. Nothing is ever deleted or cleaned up
automatically.

The frontend reads the outcome through the Tauri command
`get_startup_config_issue`, which returns `null` or
`{config_path, preserved_path, preserve_error, error, loaded}` where `loaded`
is `"last_good"` or `"defaults"`.

## Validation (`crates/common/src/config_validation.rs`)

`control_loop::validate_config` calls `config_validation::validate` first, so
`load_config`, the `validate_config` command, `UpdateConfig` and `SetRgbConfig`
all use it. It returns the first error as `<field path>: <message>` and never
clamps or substitutes values. The pre-existing checks in `validate_config` are
unchanged.

| Field | Rule |
|---|---|
| every fan `f64` (duty, curve temp/duty, hysteresis, ramps, PID terms, temp_source weights) | finite |
| Fixed `duty_percent`, curve point `duty`, PID `min_duty`/`max_duty` | 0..=100 |
| PID `min_duty` vs `max_duty` | `min <= max` (existing check still requires `<`) |
| curve `hysteresis` | >= 0 |
| curve `ramp_rate`, `ramp_down_rate` (if set) | > 0 |
| `rgb.brightness`, zone `brightness` | <= 100 |
| `rgb.fps` | <= 120 (0 allowed; the runtime treats it as 33 ms) |
| layer `opacity` | finite only (the blender clamps to 0..=1) |
| flow `delay_per_device_ms` | finite and >= 0 |
| every `EffectConfig` `f32`/`f64` (exhaustive match, incl. gradient temps, ThermalPulse min/max) | finite |
| `device_overrides[].led_count`, `devices[].led_count` | <= 255 (0 and the 80-LED channel-15 override are valid) |

RGB checks apply to `rgb.zones` and to `rgb.presets[].zones`.

`apps/gui/config.toml`, `config.example.toml` and the shipped
`assets/presets/fans.toml` values are asserted valid in unit tests.

## Persistence guard

Every command that persists a config validates first:

* `UpdateConfig` validates before saving (unchanged call, stronger validator).
* `SetRgbConfig` now validates a candidate copy and rejects with
  `Invalid RGB config: ...` before touching memory, the renderer or disk.
* `SetRgbEnabled` changes one bool of an already-valid config.
* `RenameDevice` changes only `devices[].name`; `ApplyPreset` writes curve
  values from the shipped `fans.toml` (covered by the fixture test). Neither
  has a separate validation call.

## Startup write-path audit

| Path | Trigger | Can it touch an invalid original? | Guard |
|---|---|---|---|
| `startup_config::record_last_good` -> `write_atomic(config.last-good.toml)` | primary loaded and validated at startup | no, only runs after success; writes a different file | skipped when bytes are already identical; failure is a warning |
| `startup_config::preserve_invalid` (`create_new`) | primary/legacy unusable | no, reads the original and writes a new file | never overwrites; failure reported in the issue |
| `hardware_thread::run_config_migration` -> `try_migrate_file(config_path())` rewrites a V1 primary as V2 (and `.bak`) | every start with an in-memory V1 config and an existing primary | yes: if startup fell back to last-good or defaults, the on-disk file may be invalid and would be rewritten | skipped whenever a startup issue exists (`spawn(.., skip_migration)`), logged at info |
| `save_config_to_disk` via `ConfigStore::save` | `UpdateConfig`, `SetRgbConfig`, `SetRgbEnabled`, `ApplyPreset`, `RenameDevice`, tray presets | yes, if the user acts after recovery it replaces the primary | not startup-time; the invalid original is already preserved. `UpdateConfig`/`SetRgbConfig` validate first |
| `DiskDiagnosticsStore::write_file` (`std::fs::write`) | LED marker diagnostic | no, writes under `diagnostics\` | n/a |
| `std::fs::remove_file` in `prune_old_logs` | startup | no, only `corsair-hub.log*` in `logs\` | n/a |

No other code writes `config.toml` at startup.

## Deliberately left out

* Fan-ownership / duplicate-target rules.
* Saves do not refresh `config.last-good.toml`; it only reflects what loaded
  successfully at a startup. A later bad edit that the validator accepts is not
  tracked.
* No automatic cleanup of `config.invalid-*.toml` backups.
* The legacy `cwd/config.toml` is not used when the primary exists but is invalid.
* `ApplyPreset` and `RenameDevice` are not re-validated before persisting.
