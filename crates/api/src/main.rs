use clap::Parser;
use std::process::exit;

#[derive(Parser)]
#[command(name = "keel-api", version = plumb::version!("KEEL"), about = "Serve one sealed Keel estate over HTTP")]
struct Cli {
    #[arg(
        long,
        env = "KEEL_STORE",
        help = "sqlite:PATH, a postgres:// URL or a keyword conninfo"
    )]
    store: String,
    #[arg(long, env = "KEEL_LISTEN", default_value = "0.0.0.0:8080")]
    listen: String,
    #[arg(long, env = "KEEL_PREFIX", default_value = "")]
    prefix: String,
}

#[tokio::main]
async fn main() {
    if let Err(error) = plumb::identity!("KEEL") {
        eprintln!("keel-api: {error}");
        exit(1);
    }
    let cli = Cli::parse();
    if let Err(error) = plumb::identity::ready() {
        eprintln!("keel-api: {error}");
        exit(1);
    }
    if let Err(note) = run(cli).await {
        eprintln!("keel-api: {note}");
        exit(1);
    }
}

async fn run(cli: Cli) -> Result<(), String> {
    let app = match api::open(&cli.store).await.map_err(|e| e.to_string())? {
        api::Store::Lite(wire) => api::mount(wire, &cli.prefix).await,
        api::Store::Pg(wire) => api::mount(*wire, &cli.prefix).await,
    }
    .map_err(|e| e.to_string())?;
    let listener = tokio::net::TcpListener::bind(&cli.listen)
        .await
        .map_err(|e| format!("bind {}: {e}", cli.listen))?;
    let addr = listener.local_addr().map_err(|e| e.to_string())?;
    eprintln!("keel-api: ready on http://{addr}{}", cli.prefix);
    axum::serve(listener, app)
        .with_graceful_shutdown(stop())
        .await
        .map_err(|e| e.to_string())
}

#[cfg(unix)]
async fn stop() {
    use tokio::signal::unix::{SignalKind, signal};
    let Ok(mut term) = signal(SignalKind::terminate()) else {
        let _ = tokio::signal::ctrl_c().await;
        return;
    };
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = term.recv() => {}
    }
}

#[cfg(not(unix))]
async fn stop() {
    let _ = tokio::signal::ctrl_c().await;
}
