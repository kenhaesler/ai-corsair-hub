use std::env;
use std::ffi::OsString;
use std::path::Path;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use corsair_common::config::AppConfig;
use corsair_common::hardware_ownership::try_acquire_hardware_ownership;
use corsair_fancontrol::control_loop::{self, ControlLoop};
use corsair_hid::DeviceScanner;
use corsair_service::{
    Command, SERVICE_DISPLAY_NAME, SERVICE_NAME, USAGE, log_dir, parse_args, resolve_config_path,
};
use tracing::{error, info};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;
use windows_service::define_windows_service;
use windows_service::service::{
    ServiceAccess, ServiceControl, ServiceControlAccept, ServiceErrorControl, ServiceExitCode,
    ServiceInfo, ServiceStartType, ServiceState, ServiceStatus, ServiceType,
};
use windows_service::service_control_handler::{
    self, ServiceControlHandlerResult, ServiceStatusHandle,
};
use windows_service::service_dispatcher;
use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

const START_WAIT_HINT: Duration = Duration::from_secs(120);
const STOP_WAIT_HINT: Duration = Duration::from_secs(30);
const UNINSTALL_STOP_TIMEOUT: Duration = Duration::from_secs(30);

define_windows_service!(ffi_service_main, service_main);

fn main() -> Result<()> {
    let command = parse_args(env::args_os().skip(1)).map_err(|message| anyhow!(message))?;
    let program_data = env::var_os("ProgramData");
    let current_dir = env::current_dir().context("cannot determine current directory")?;

    match command {
        Command::Service { config } => {
            let config_path =
                resolve_config_path(config.as_deref(), program_data.as_deref(), &current_dir);
            service_dispatcher::start(SERVICE_NAME, ffi_service_main).with_context(|| {
                format!(
                    "failed to connect to the Windows Service Manager; use --console for an interactive run (config: {})",
                    config_path.display()
                )
            })?;
        }
        Command::Console { config } => {
            let config_path =
                resolve_config_path(config.as_deref(), program_data.as_deref(), &current_dir);
            run_console(&config_path)?;
        }
        Command::Install { config } => {
            let config_path =
                resolve_config_path(config.as_deref(), program_data.as_deref(), &current_dir);
            install_service(&config_path)?;
        }
        Command::Uninstall => uninstall_service()?,
        Command::Help => println!("{USAGE}"),
        Command::Version => println!("corsair-service {}", env!("CARGO_PKG_VERSION")),
    }

    Ok(())
}

fn service_main(_arguments: Vec<OsString>) {
    if let Err(err) = run_service_entry() {
        error!(error = %err, "Windows service terminated with an error");
    }
}

fn run_service_entry() -> Result<()> {
    let shutdown = Arc::new(AtomicBool::new(false));
    let status_cell = Arc::new(OnceLock::<ServiceStatusHandle>::new());
    let handler_shutdown = Arc::clone(&shutdown);
    let handler_status = Arc::clone(&status_cell);

    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                if !handler_shutdown.swap(true, Ordering::SeqCst)
                    && let Some(status_handle) = handler_status.get()
                {
                    let _ = set_status(
                        status_handle,
                        ServiceState::StopPending,
                        ServiceControlAccept::empty(),
                        ServiceExitCode::Win32(0),
                        1,
                        STOP_WAIT_HINT,
                    );
                }
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)
        .context("failed to register the service control handler")?;
    set_status(
        &status_handle,
        ServiceState::StartPending,
        ServiceControlAccept::empty(),
        ServiceExitCode::Win32(0),
        1,
        START_WAIT_HINT,
    )?;
    status_cell
        .set(status_handle)
        .map_err(|_| anyhow!("service status handle was initialized twice"))?;
    if shutdown.load(Ordering::SeqCst) {
        set_status(
            &status_handle,
            ServiceState::StopPending,
            ServiceControlAccept::empty(),
            ServiceExitCode::Win32(0),
            1,
            STOP_WAIT_HINT,
        )?;
    }

    let result = run_service_worker(&status_handle, shutdown);
    let exit_code = if result.is_ok() {
        ServiceExitCode::Win32(0)
    } else {
        ServiceExitCode::ServiceSpecific(1)
    };
    let status_result = set_status(
        &status_handle,
        ServiceState::Stopped,
        ServiceControlAccept::empty(),
        exit_code,
        0,
        Duration::ZERO,
    );

    result.and(status_result)
}

fn run_service_worker(
    status_handle: &ServiceStatusHandle,
    shutdown: Arc<AtomicBool>,
) -> Result<()> {
    let program_data = env::var_os("ProgramData");
    let current_dir = env::current_dir().context("cannot determine current directory")?;
    let command = parse_args(env::args_os().skip(1)).map_err(|message| anyhow!(message))?;
    let config_override = match command {
        Command::Service { config } => config,
        _ => bail!("the installed service was launched with an invalid command"),
    };
    let config_path = resolve_config_path(
        config_override.as_deref(),
        program_data.as_deref(),
        &current_dir,
    );
    let log_path = log_dir(program_data.as_deref());
    let filter = config_log_filter(&config_path);
    let _log_guard = init_service_tracing(&log_path, filter)?;

    info!(
        version = env!("CARGO_PKG_VERSION"),
        config = %config_path.display(),
        logs = %log_path.display(),
        "Windows service starting"
    );

    let result = (|| {
        let _hardware_owner = try_acquire_hardware_ownership()
            .context("cannot start fan control because hardware ownership is unavailable")?;
        let config = control_loop::load_config(&config_path)
            .with_context(|| format!("failed to load service config {}", config_path.display()))?;
        let mut control = build_control_loop(config, Arc::clone(&shutdown))?;

        if !shutdown.load(Ordering::SeqCst) {
            set_status(
                status_handle,
                ServiceState::Running,
                ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
                ServiceExitCode::Win32(0),
                0,
                Duration::ZERO,
            )?;
        }

        control.run().context("fan control loop failed")?;
        info!("Windows service stopped; hardware mode restored");
        Ok(())
    })();
    if let Err(err) = &result {
        error!(error = %err, "Windows service worker failed");
    }
    result
}

fn set_status(
    handle: &ServiceStatusHandle,
    current_state: ServiceState,
    controls_accepted: ServiceControlAccept,
    exit_code: ServiceExitCode,
    checkpoint: u32,
    wait_hint: Duration,
) -> Result<()> {
    handle
        .set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state,
            controls_accepted,
            exit_code,
            checkpoint,
            wait_hint,
            process_id: None,
        })
        .context("failed to update Windows service status")
}

fn run_console(config_path: &Path) -> Result<()> {
    let config = control_loop::load_config(config_path)
        .with_context(|| format!("failed to load config {}", config_path.display()))?;
    init_console_tracing(&config.general.log_level)?;

    println!(
        "ai-corsair-hub v{} - Fan Control Service",
        env!("CARGO_PKG_VERSION")
    );
    println!(
        "  Config: {} ({} fan groups)",
        config_path.display(),
        config.fan_groups.len()
    );
    println!("  Press Ctrl+C to stop\n");

    let _hardware_owner = try_acquire_hardware_ownership()
        .context("cannot start fan control because hardware ownership is unavailable")?;

    let shutdown = Arc::new(AtomicBool::new(false));
    let handler_shutdown = Arc::clone(&shutdown);
    ctrlc::set_handler(move || {
        eprintln!("\n  Shutting down...");
        handler_shutdown.store(true, Ordering::SeqCst);
    })
    .context("failed to register Ctrl+C handler")?;

    let mut control = build_control_loop(config, shutdown)?;
    control.run().context("fan control loop failed")?;
    println!("  Shutdown complete. Hardware mode restored.");
    Ok(())
}

fn build_control_loop(config: AppConfig, shutdown: Arc<AtomicBool>) -> Result<ControlLoop> {
    let scanner = DeviceScanner::new().context("failed to initialize HID device scanner")?;
    ControlLoop::build(config, shutdown, &scanner).context("failed to initialize fan control")
}

fn config_log_filter(config_path: &Path) -> String {
    let level = std::fs::read_to_string(config_path)
        .ok()
        .and_then(|contents| toml::from_str::<toml::Value>(&contents).ok())
        .and_then(|config| {
            config
                .get("general")?
                .get("log_level")?
                .as_str()
                .map(str::to_ascii_lowercase)
        })
        .filter(|level| {
            matches!(
                level.as_str(),
                "trace" | "debug" | "info" | "warn" | "error"
            )
        })
        .unwrap_or_else(|| "info".to_string());
    format!("corsair={level}")
}

fn init_service_tracing(log_path: &Path, default_filter: String) -> Result<WorkerGuard> {
    std::fs::create_dir_all(log_path)
        .with_context(|| format!("failed to create log directory {}", log_path.display()))?;
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));
    let file_appender = tracing_appender::rolling::daily(log_path, "corsair-service.log");
    let (writer, guard) = tracing_appender::non_blocking(file_appender);
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(false)
        .with_writer(writer)
        .try_init()
        .map_err(|err| anyhow!("failed to initialize service logging: {err}"))?;
    Ok(guard)
}

fn init_console_tracing(log_level: &str) -> Result<()> {
    let default_filter = format!("corsair={log_level}");
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter)),
        )
        .try_init()
        .map_err(|err| anyhow!("failed to initialize console logging: {err}"))
}

fn install_service(config_path: &Path) -> Result<()> {
    ensure_config(config_path)?;
    let config_path = config_path
        .canonicalize()
        .with_context(|| format!("failed to resolve config path {}", config_path.display()))?;
    let executable_path = env::current_exe()
        .context("failed to locate service executable")?
        .canonicalize()
        .context("failed to resolve service executable path")?;

    let manager = ServiceManager::local_computer(
        None::<&str>,
        ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
    )
    .context("failed to open the Windows Service Manager; run from an elevated terminal")?;
    let service_info = ServiceInfo {
        name: OsString::from(SERVICE_NAME),
        display_name: OsString::from(SERVICE_DISPLAY_NAME),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path,
        launch_arguments: vec![
            OsString::from("--config"),
            config_path.as_os_str().to_owned(),
        ],
        dependencies: vec![],
        account_name: None,
        account_password: None,
    };
    let service = manager
        .create_service(
            &service_info,
            ServiceAccess::CHANGE_CONFIG | ServiceAccess::DELETE,
        )
        .context("failed to install service; it may already exist or elevation is required")?;
    if let Err(err) = service.set_description(
        "Controls Corsair fan and pump speeds from system temperatures and restores hardware mode on shutdown.",
    ) {
        let rollback = service.delete();
        bail!(
            "service description could not be set ({err}); installation rollback: {}",
            if rollback.is_ok() {
                "successful"
            } else {
                "failed; remove the partially installed service manually"
            }
        );
    }

    println!("Installed {SERVICE_DISPLAY_NAME} ({SERVICE_NAME}).");
    println!("Config: {}", config_path.display());
    println!("Start it with: Start-Service {SERVICE_NAME}");
    Ok(())
}

fn ensure_config(config_path: &Path) -> Result<()> {
    if config_path.exists() {
        control_loop::load_config(config_path).with_context(|| {
            format!(
                "existing config is invalid and was not modified: {}",
                config_path.display()
            )
        })?;
        return Ok(());
    }

    let serialized = toml::to_string_pretty(&AppConfig::default())
        .context("failed to serialize default service config")?;
    corsair_common::atomic_write::write_atomic(config_path, serialized.as_bytes())
        .with_context(|| format!("failed to create default config {}", config_path.display()))?;
    println!("Created default config: {}", config_path.display());
    Ok(())
}

fn uninstall_service() -> Result<()> {
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
        .context("failed to open the Windows Service Manager; run from an elevated terminal")?;
    let service = manager
        .open_service(
            SERVICE_NAME,
            ServiceAccess::QUERY_STATUS | ServiceAccess::STOP | ServiceAccess::DELETE,
        )
        .context("failed to open the installed service")?;

    let state = service
        .query_status()
        .context("failed to query service status")?
        .current_state;
    if state != ServiceState::Stopped {
        if state != ServiceState::StopPending {
            service.stop().context("failed to request service stop")?;
        }
        wait_until_stopped(&service)?;
    }

    service
        .delete()
        .context("failed to mark service for deletion")?;
    println!("Uninstalled {SERVICE_DISPLAY_NAME} ({SERVICE_NAME}).");
    Ok(())
}

fn wait_until_stopped(service: &windows_service::service::Service) -> Result<()> {
    let deadline = Instant::now() + UNINSTALL_STOP_TIMEOUT;
    loop {
        let status = service
            .query_status()
            .context("failed while waiting for the service to stop")?;
        if status.current_state == ServiceState::Stopped {
            return Ok(());
        }
        if Instant::now() >= deadline {
            bail!(
                "service did not stop within {} seconds; it was not uninstalled",
                UNINSTALL_STOP_TIMEOUT.as_secs()
            );
        }
        thread::sleep(Duration::from_millis(250));
    }
}
