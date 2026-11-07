//! The code tool's "Connected services" section: the accounts the user
//! added and the kinds of resource imported from them, appended to the script
//! API in the tool's description.

use std::fmt::Write;

use cohesion_core::model::{AccountHost, Connector, ConnectorAccount};
use cohesion_db::{
    CommandError, DatabaseContext, ListConnectorAccountRowsCommand, ListConnectorsCommand,
};

/// The "Connected services" section of a code tool's description; empty
/// when the user has added no account.
#[derive(Clone, Default)]
pub struct ConnectedServices(String);

impl ConnectedServices {
    /// The section for the accounts the user has added now.
    pub async fn load(database: &DatabaseContext) -> Result<Self, CommandError> {
        let connectors = database.send(ListConnectorsCommand {}).await?;
        let accounts = database.send(ListConnectorAccountRowsCommand {}).await?;
        Ok(Self(section(&connectors, &accounts)))
    }

    /// The section's comment lines, ending in a newline; empty when the
    /// user has added no account.
    pub(crate) fn section(&self) -> &str {
        &self.0
    }
}

/// For each connector with an account, one comment line, one per kind of
/// resource it declares, and one per account.
fn section(connectors: &[Connector], accounts: &[ConnectorAccount]) -> String {
    if accounts.is_empty() {
        return String::new();
    }
    let mut text = String::from(
        "// Connected services: accounts the user added. Their items are imported as read-only\n\
         // resources, read with `workspace.resources({ kind })`; a connector's `kinds` (the \
         `connectors` module) say each one's contents.\n",
    );
    for connector in connectors {
        let mine: Vec<&ConnectorAccount> = accounts
            .iter()
            .filter(|account| account.connector_id == connector.id)
            .collect();
        if mine.is_empty() {
            continue;
        }
        let identifier = one_line(&connector.identifier);
        _ = writeln!(
            text,
            "// - {} (connector {identifier})",
            one_line(&connector.title)
        );
        for kind in &connector.kinds {
            _ = writeln!(
                text,
                "//     kind \"{identifier}.{}\" ({})",
                one_line(&kind.identifier),
                one_line(&kind.title),
            );
        }
        for account in mine {
            let host = match account.host {
                AccountHost::Extension { .. } => "",
                AccountHost::Orphaned => {
                    ", orphaned: its resources stay, its items can't be browsed"
                }
            };
            _ = writeln!(
                text,
                "//     account \"{}\" (connectors.account({}){host})",
                one_line(&account.description),
                account.id,
            );
        }
    }
    text
}

/// `text` with its whitespace runs, line breaks included, collapsed to one
/// space, so text an extension supplied stays inside its comment line.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
