use cohesion_core::model::ExtensionPermissionKind;

use crate::script_api::{
    ACCOUNTS, ACTORS, AGENT, AGENTS, ARTICLES, CONNECTORS, CONTACTS, CORE, CUES, IMAGES,
    INTELLIGENCE, LOG, NETWORK, NOTIFICATIONS, RESOURCES, REWIND, SERVICES, STORAGE, ScriptModule,
    TIMER, USAGE, VOCABULARY,
};

#[derive(PartialEq, Copy, Clone)]
pub enum Mount {
    Cohesion,
    Intelligence,
    Log,
    Network,
    /// `accounts`, the accounts the user added for the extension, mounted
    /// by the `oauth` permission.
    Accounts,
    Timer,
    /// `agent`, for an agent the user is talking to.
    Agent,
    /// `connectors`, for an agent: the user's connector accounts and their
    /// services.
    Connectors,
    /// `images`, for an agent's turns (never its cues): attaches
    /// images to the messages it sends the user. No extension permission
    /// maps to it.
    Images,
    /// `actors`, the actors clients offer and their sessions: reads them, and
    /// starts, talks to and stops sessions. Mounted for an agent's turns (never
    /// its cues), and by the `actors` permission.
    Actors,
    /// `services`, whose `use` reaches the functions an installed service
    /// declares; with a read-only database, only those declared `readOnly`
    /// can be called.
    Services,
    /// `storage`, the private key/value store of the sandbox's extension or
    /// of the agent whose job is running. Needs no permission.
    Storage,
    /// `usage`, the tokens Cohesion's models used and what they cost.
    Usage,
    /// `notifications`, which sends the user a notification: a write of the
    /// `Notifications` area, the one every agent role may write.
    Notifications,
    /// No global: lets `Article.resnapshot` fetch a page. Only the contexts the user drives mount it (`cohesion run` and
    /// agents' conversations); no extension permission maps to it.
    ArticleFetch,
    /// No global: lets `cohesion.sleep` pause the script. Only the contexts
    /// that hold their own thread or that the user can stop mount it
    /// (`cohesion run`, agents' conversations, and every extension beside
    /// `log` and `storage`, needing no permission of its own), never the
    /// pool cues share.
    Sleep,
}

impl Mount {
    /// The global an extension permission mounts, if any.
    pub fn for_permission(permission: &ExtensionPermissionKind) -> Option<Mount> {
        match permission {
            ExtensionPermissionKind::GlobalReadOnly | ExtensionPermissionKind::GlobalReadWrite => {
                Some(Mount::Cohesion)
            }
            ExtensionPermissionKind::Network => Some(Mount::Network),
            ExtensionPermissionKind::Timer => Some(Mount::Timer),
            // Lets the daemon call `registryResults` and `registryLookup`; no global.
            ExtensionPermissionKind::Registry => None,
            // Also lets the daemon call the sign-in functions.
            ExtensionPermissionKind::OAuth => Some(Mount::Accounts),
            // Lets the daemon call the import functions; no global.
            ExtensionPermissionKind::Connector => None,
            // Lets clients call `serviceRequest`; no global.
            ExtensionPermissionKind::Service => None,
            // Lets clients call `serviceRequest`, and nothing else; no global.
            ExtensionPermissionKind::ServiceIsolated => None,
            ExtensionPermissionKind::Services => Some(Mount::Services),
            ExtensionPermissionKind::Usage => Some(Mount::Usage),
            ExtensionPermissionKind::Intelligence => Some(Mount::Intelligence),
            ExtensionPermissionKind::Notifications => Some(Mount::Notifications),
            ExtensionPermissionKind::Actors => Some(Mount::Actors),
        }
    }

    /// The global this mounts, as scripts call it, or `None` for a mount
    /// that only lets an existing global do more.
    pub fn global(self) -> Option<&'static str> {
        match self {
            Mount::Cohesion => Some("cohesion"),
            Mount::Intelligence => Some("intelligence"),
            Mount::Log => Some("log"),
            Mount::Network => Some("network"),
            Mount::Accounts => Some("accounts"),
            Mount::Timer => Some("timer"),
            Mount::Agent => Some("agent"),
            Mount::Connectors => Some("connectors"),
            Mount::Images => Some("images"),
            Mount::Actors => Some("actors"),
            Mount::Services => Some("services"),
            Mount::Storage => Some("storage"),
            Mount::Usage => Some("usage"),
            Mount::Notifications => Some("notifications"),
            Mount::ArticleFetch | Mount::Sleep => None,
        }
    }

    /// The script API modules documenting what this mounts, so a sandbox's
    /// documentation follows from its mounts and can't promise a global it
    /// lacks.
    pub fn modules(self) -> &'static [ScriptModule] {
        match self {
            Mount::Cohesion => &[
                CORE, AGENTS, ARTICLES, RESOURCES, CONTACTS, REWIND, VOCABULARY,
            ],
            Mount::Intelligence => &[INTELLIGENCE],
            Mount::Log => &[LOG],
            Mount::Network => &[NETWORK],
            Mount::Accounts => &[ACCOUNTS],
            Mount::Timer => &[TIMER],
            Mount::Agent => &[AGENT, CUES],
            Mount::Connectors => &[CONNECTORS],
            Mount::Images => &[IMAGES],
            Mount::Actors => &[ACTORS],
            Mount::Services => &[SERVICES],
            Mount::Storage => &[STORAGE],
            Mount::Usage => &[USAGE],
            Mount::Notifications => &[NOTIFICATIONS],
            Mount::ArticleFetch | Mount::Sleep => &[],
        }
    }
}
