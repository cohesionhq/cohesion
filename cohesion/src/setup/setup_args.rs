use std::path::PathBuf;

use clap::Args;

#[derive(Args)]
pub struct SetupArgs {
    /// The setup code cohesiond printed when it started.
    #[arg(required_unless_present = "local")]
    pub setup_code: Option<String>,
    /// How cohesiond will list this client. Defaults to this computer's hostname.
    #[arg(long)]
    pub name: Option<String>,
    /// Connect to a cohesiond on this machine through its Unix socket instead of
    /// registering with a setup code. Uses cohesiond's default socket when no path is given.
    #[arg(long, value_name = "SOCKET", num_args = 0..=1, conflicts_with = "setup_code")]
    pub local: Option<Option<PathBuf>>,
}
