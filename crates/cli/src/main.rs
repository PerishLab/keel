use clap::{Parser, Subcommand};
use keel::adapt::Error;
use std::path::PathBuf;
use std::process::exit;

#[derive(Parser)]
#[command(name = "keel", version = plumb::version!("KEEL"), about = "Operator ceremonies against a Keel store")]
struct Cli {
    #[command(subcommand)]
    deed: Deed,
}

#[derive(Subcommand)]
enum Deed {
    #[command(
        about = "Seal a vacant estate from a manifest and write its sudo token to a new custody file"
    )]
    Bootstrap {
        #[arg(
            long,
            env = "KEEL_STORE",
            help = "sqlite:PATH, a postgres:// URL or a keyword conninfo"
        )]
        store: String,
        #[arg(long)]
        model: PathBuf,
        #[arg(long)]
        custody: PathBuf,
    },
    #[command(about = "Report whether the estate is vacant, sealed or unknown")]
    Status {
        #[arg(long, env = "KEEL_STORE")]
        store: String,
    },
    #[command(about = "Print the manifest of a model file or of a sealed estate")]
    Model {
        #[arg(required_unless_present = "store", conflicts_with = "store")]
        file: Option<PathBuf>,
        #[arg(long)]
        store: Option<String>,
    },
}

#[tokio::main]
async fn main() {
    if let Err(error) = plumb::identity!("KEEL") {
        eprintln!("keel: {error}");
        exit(1);
    }
    let cli = Cli::parse();
    if let Err(error) = plumb::identity::ready() {
        eprintln!("keel: {error}");
        exit(1);
    }
    match run(cli.deed).await {
        Ok(text) => println!("{text}"),
        Err(error) => {
            eprintln!("keel: {error}");
            exit(1);
        }
    }
}

async fn run(deed: Deed) -> Result<String, Error> {
    match deed {
        Deed::Bootstrap {
            store,
            model,
            custody,
        } => {
            let graph = api::model(&model)?;
            match api::open(&store).await? {
                api::Store::Lite(wire) => api::seal(wire, graph, &custody).await?,
                api::Store::Pg(wire) => api::seal(*wire, graph, &custody).await?,
            }
            Ok(format!("sealed; token in {}", custody.display()))
        }
        Deed::Status { store } => Ok(match api::open(&store).await? {
            api::Store::Lite(wire) => api::status(wire).await?,
            api::Store::Pg(wire) => api::status(*wire).await?,
        }
        .to_string()),
        Deed::Model {
            file: Some(file), ..
        } => api::model(&file)?.write(),
        Deed::Model { store, .. } => {
            let store = store.unwrap_or_default();
            let graph = match api::open(&store).await? {
                api::Store::Lite(mut wire) => keel::estate::read(&mut wire).await?,
                api::Store::Pg(mut wire) => keel::estate::read(&mut *wire).await?,
            };
            graph.write()
        }
    }
}
