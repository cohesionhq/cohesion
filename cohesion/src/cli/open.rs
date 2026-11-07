use clap::Args;
use cohesion_core::protocol::{AboutRequest, ListWorkspacesRequest, WebBundleRequest};

use crate::{
    cli_context::Context,
    cli_error::CliError,
    web_proxy::{self, ClientInfo, Mode, Options, launch_token, static_assets::Assets},
};

/// The environment variable holding the token `--dev` expects the dev server to present.
const DEV_TOKEN_VARIABLE: &str = "COHESION_DEV_TOKEN";

#[derive(Args)]
pub struct OpenArgs {
    /// Port to serve on; 7343, or a free one if that's taken, when omitted.
    #[arg(long)]
    port: Option<u16>,
    /// Print the URL without opening a browser.
    #[arg(long)]
    no_browser: bool,
    /// Serve only the API, for the web UI's dev server, which presents the token in
    /// `COHESION_DEV_TOKEN` (see `scripts/web-dev.sh`).
    #[arg(long)]
    dev: bool,
    /// Replace the saved launch token, so URLs and browsers given the old one stop working.
    #[arg(long, conflicts_with = "dev")]
    reset_token: bool,
    /// Stop serving when stdin closes, for apps that run this as a child and pass it a pipe.
    #[arg(long)]
    exit_with_parent: bool,
}

/// Serves the web UI for this client's cohesiond until Ctrl-C.
pub async fn run(ctx: &Context, args: OpenArgs) -> Result<(), CliError> {
    let about = ctx.call(AboutRequest).await?;
    let workspaces = ctx.call(ListWorkspacesRequest).await?;
    let client = ClientInfo {
        workspace: ctx.current_workspace(&workspaces)?,
        target: ctx.target(),
    };
    let token = if args.dev {
        std::env::var(DEV_TOKEN_VARIABLE).map_err(|_| CliError::MissingDevToken)?
    } else {
        launch_token::saved(ctx.storage_path(), args.reset_token).map_err(CliError::Storage)?
    };
    let options = Options {
        port: args.port,
        token,
        mode: if args.dev {
            Mode::Dev
        } else {
            Mode::Launch {
                browser: !args.no_browser,
                assets: Assets::unpack(&ctx.call(WebBundleRequest).await?)?,
            }
        },
        exit_with_parent: args.exit_with_parent,
    };
    web_proxy::serve(ctx.connection().clone(), client, &about.hostname, options).await
}
