use clap::{Args, Subcommand};

/// A command named for a thing, whose subcommands are verbs. Without a verb it takes its
/// default action, usually listing.
#[derive(Args)]
#[command(args_conflicts_with_subcommands = true)]
pub struct Noun<T: Subcommand> {
    #[command(subcommand)]
    pub verb: Option<T>,
}
