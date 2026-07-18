use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

pub const SERVICE_NAME: &str = "CorsairHub";
pub const SERVICE_DISPLAY_NAME: &str = "Corsair Hub Fan Control";
pub const APP_DIRECTORY_NAME: &str = "ai-corsair-hub";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Service { config: Option<PathBuf> },
    Console { config: Option<PathBuf> },
    Install { config: Option<PathBuf> },
    Uninstall,
    Help,
    Version,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Mode {
    Service,
    Console,
    Install,
    Uninstall,
    Help,
    Version,
}

pub fn parse_args<I>(args: I) -> Result<Command, String>
where
    I: IntoIterator<Item = OsString>,
{
    let args: Vec<OsString> = args.into_iter().collect();
    let mut mode = Mode::Service;
    let mut explicit_mode = false;
    let mut config = None;
    let mut index = 0;

    while index < args.len() {
        let arg = &args[index];
        if arg == "--config" {
            if config.is_some() {
                return Err("--config may only be specified once".to_string());
            }
            index += 1;
            let value = args
                .get(index)
                .ok_or_else(|| "--config requires a path".to_string())?;
            if matches!(
                value.to_str(),
                Some(
                    "--console"
                        | "--install"
                        | "--uninstall"
                        | "--help"
                        | "-h"
                        | "--version"
                        | "-V"
                )
            ) {
                return Err("--config requires a path".to_string());
            }
            config = Some(PathBuf::from(value));
        } else {
            let candidate = match arg.to_str() {
                Some("--console") => Some(Mode::Console),
                Some("install" | "--install") => Some(Mode::Install),
                Some("uninstall" | "--uninstall") => Some(Mode::Uninstall),
                Some("help" | "--help" | "-h") => Some(Mode::Help),
                Some("--version" | "-V") => Some(Mode::Version),
                _ => None,
            };

            let candidate =
                candidate.ok_or_else(|| format!("unknown argument: {}", arg.to_string_lossy()))?;
            if explicit_mode {
                return Err("only one command or run mode may be specified".to_string());
            }
            mode = candidate;
            explicit_mode = true;
        }
        index += 1;
    }

    match mode {
        Mode::Service => Ok(Command::Service { config }),
        Mode::Console => Ok(Command::Console { config }),
        Mode::Install => Ok(Command::Install { config }),
        Mode::Uninstall if config.is_none() => Ok(Command::Uninstall),
        Mode::Help if config.is_none() => Ok(Command::Help),
        Mode::Version if config.is_none() => Ok(Command::Version),
        Mode::Uninstall => Err("--config is not valid with uninstall".to_string()),
        Mode::Help => Err("--config is not valid with help".to_string()),
        Mode::Version => Err("--config is not valid with --version".to_string()),
    }
}

pub fn default_data_dir(program_data: Option<&OsStr>) -> PathBuf {
    let base = program_data
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"));
    base.join(APP_DIRECTORY_NAME)
}

pub fn resolve_config_path(
    override_path: Option<&Path>,
    program_data: Option<&OsStr>,
    current_dir: &Path,
) -> PathBuf {
    match override_path {
        Some(path) if path.is_absolute() => path.to_path_buf(),
        Some(path) => current_dir.join(path),
        None => default_data_dir(program_data).join("config.toml"),
    }
}

pub fn log_dir(program_data: Option<&OsStr>) -> PathBuf {
    default_data_dir(program_data).join("logs")
}

pub const USAGE: &str = "\
Corsair Hub Fan Control Service

Usage:
  corsair-service                         Run under the Windows Service Manager
  corsair-service --console [--config P] Run interactively for development
  corsair-service install [--config P]   Install the automatic Windows service
  corsair-service uninstall              Stop and uninstall the Windows service
  corsair-service --help                 Show this help
  corsair-service --version              Show the version

The default config is %ProgramData%\\ai-corsair-hub\\config.toml.
Service logs are written under %ProgramData%\\ai-corsair-hub\\logs.
Install and uninstall require an elevated terminal.";

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn defaults_to_service_mode() {
        assert_eq!(
            parse_args(args(&[])).unwrap(),
            Command::Service { config: None }
        );
    }

    #[test]
    fn parses_console_and_config_in_either_order() {
        let expected = Command::Console {
            config: Some(PathBuf::from("custom.toml")),
        };
        assert_eq!(
            parse_args(args(&["--console", "--config", "custom.toml"])).unwrap(),
            expected
        );
        assert_eq!(
            parse_args(args(&["--config", "custom.toml", "--console"])).unwrap(),
            expected
        );
    }

    #[test]
    fn parses_install_and_uninstall_aliases() {
        assert_eq!(
            parse_args(args(&["--install"])).unwrap(),
            Command::Install { config: None }
        );
        assert_eq!(
            parse_args(args(&["uninstall"])).unwrap(),
            Command::Uninstall
        );
    }

    #[test]
    fn rejects_ambiguous_or_incomplete_arguments() {
        assert!(parse_args(args(&["install", "--console"])).is_err());
        assert!(parse_args(args(&["--config"])).is_err());
        assert!(parse_args(args(&["--config", "--console"])).is_err());
        assert!(parse_args(args(&["uninstall", "--config", "x"])).is_err());
        assert!(parse_args(args(&["--unknown"])).is_err());
    }

    #[test]
    fn service_paths_use_program_data() {
        let program_data = OsStr::new(r"D:\SharedData");
        assert_eq!(
            default_data_dir(Some(program_data)),
            PathBuf::from(r"D:\SharedData\ai-corsair-hub")
        );
        assert_eq!(
            log_dir(Some(program_data)),
            PathBuf::from(r"D:\SharedData\ai-corsair-hub\logs")
        );
    }

    #[test]
    fn empty_program_data_has_absolute_fallback() {
        assert_eq!(
            default_data_dir(Some(OsStr::new(""))),
            PathBuf::from(r"C:\ProgramData\ai-corsair-hub")
        );
    }

    #[test]
    fn relative_config_override_is_anchored_to_cwd() {
        assert_eq!(
            resolve_config_path(
                Some(Path::new("settings.toml")),
                None,
                Path::new(r"C:\work")
            ),
            PathBuf::from(r"C:\work\settings.toml")
        );
    }
}
