
use anyhow::Result;
use vigil::config::Config;
use vigil::tui;
use vigil::tui::app::App;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    // Parse CLI config + TOML config
    let config = Config::load();

    // Setup logging
    let _guard = setup_logging(&config)?;

    tracing::info!("Starting Vigil...");

    // TUI setup
    let mut terminal = tui::init()?;
    
    let mut app = App::new(config);
    let res = app.run(&mut terminal).await;

    tui::restore()?;

    if let Err(e) = res {
        eprintln!("Error running Vigil: {:?}", e);
    }

    Ok(())
}

/// Sets up non-blocking file logging based on the current timestamp
fn setup_logging(_config: &Config) -> Result<WorkerGuard> {
    let now = chrono::Local::now();
    let log_filename = format!("vigil_{}.log", now.format("%Y%m%d_%H%M%S"));
    
    let log_dir = std::path::Path::new("/tmp/vigil");
    std::fs::create_dir_all(log_dir).ok();
    
    // We only log to a file, omitting colored ANSI codes to keep the log clean
    let file_appender = tracing_appender::rolling::never(log_dir, log_filename);
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let format_layer = fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false) // No ANSI in the log file
        .with_thread_ids(true)
        .with_target(true);
        
    let filter_layer = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("debug"));

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(format_layer)
        .init();

    Ok(guard)
}
