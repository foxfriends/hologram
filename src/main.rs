use std::path::PathBuf;

mod daemon;

fn socket_file() -> PathBuf {
    std::env::temp_dir().join("hologramd.socket")
}

/// Create holographic files, projecting the output of an arbitrary command.
///
/// Use at your own risk.
#[derive(structopt::StructOpt)]
enum Args {
    /// Runs the daemon. This is expected to be run and managed by systemd.
    Start,
    /// Create a new hologram.
    Add {
        /// The path at which to create the hologram.
        dest: PathBuf,
        /// The command to run that produces the contents of the hologram.
        ///
        /// Include any arguments required.
        cmd: Vec<String>,
    },
    Remove {
        /// The path to the hologram to stop producing.
        dest: PathBuf,
    },
}

#[paw::main]
#[tokio::main]
async fn main(args: Args) -> anyhow::Result<()> {
    match args {
        Args::Start => daemon::daemon().await,
        Args::Add { dest, cmd } => {
            let code = daemon::Cmd::add(dest, cmd).await?;
            std::process::exit(code);
        }
        Args::Remove { dest } => {
            let code = daemon::Cmd::remove(dest).await?;
            std::process::exit(code);
        }
    }
}
