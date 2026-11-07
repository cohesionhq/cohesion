use cohesion_core::protocol::BeginPairingRequest;

use crate::{cli_context::Context, cli_error::CliError};

/// Opens a pairing window on cohesiond and prints the setup code for `cohesion setup`.
pub async fn pair(ctx: &Context) -> Result<(), CliError> {
    let pairing = ctx.call(BeginPairingRequest).await?;
    println!("{}", pairing.code);
    eprintln!("This code works once, for 10 minutes.");
    Ok(())
}
