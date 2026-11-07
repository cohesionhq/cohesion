use std::borrow::Cow;

use cohesion_core::model::Initiative;
use cohesion_db::{GuardedDatabaseContext, WriteArea};

use crate::globals::Mount;

/// What an agent's script runs as, which is all that decides what it may
/// reach: the one place an agent's rights are set. A capability a role
/// lacks is a mount it doesn't get, or a database that refuses the write,
/// never a check inside a mount.
#[derive(Clone, Copy)]
pub enum AgentRole {
    /// A turn of the agent's conversation or a fork of it, with the rights of whoever started its chain: the
    /// user's, everything; an event's, reading (its database refuses writes
    /// but notifications, and it mounts no `network`). Both are documented
    /// alike ([`AgentRole::DOCUMENTED`]), so every turn shares one cached
    /// prompt.
    Turn(Initiative),
    /// A cue's function, deciding whether its fork wakes: reads `cohesion`
    /// and sends notifications, nothing more.
    Cue,
}

/// What a turn the user started mounts; every turn's code tool documents
/// exactly these.
const USER_TURN_MOUNTS: &[Mount] = &[
    Mount::Cohesion,
    Mount::Network,
    Mount::Agent,
    Mount::Connectors,
    Mount::Actors,
    Mount::Images,
    Mount::Services,
    Mount::Storage,
    Mount::Notifications,
    Mount::ArticleFetch,
    Mount::Sleep,
];

/// What a turn an event started mounts: a user's turn's, without reaching
/// the web.
const EVENT_TURN_MOUNTS: &[Mount] = &[
    Mount::Cohesion,
    Mount::Agent,
    Mount::Connectors,
    Mount::Actors,
    Mount::Images,
    Mount::Services,
    Mount::Storage,
    Mount::Notifications,
    Mount::Sleep,
];

/// What a cue mounts: reading, read-only services, the agent's storage and
/// notifications, so deciding whether to wake has no other side effect.
const CUE_MOUNTS: &[Mount] = &[
    Mount::Cohesion,
    Mount::Services,
    Mount::Storage,
    Mount::Notifications,
];

impl AgentRole {
    /// What every turn's code tool declares, whoever started it, so the
    /// tool definitions never differ between turns.
    pub const DOCUMENTED: AgentRole = AgentRole::Turn(Initiative::User);

    /// The globals and abilities a script of this role mounts.
    pub fn mounts(self) -> Cow<'static, [Mount]> {
        Cow::Borrowed(match self {
            Self::Turn(Initiative::User) => USER_TURN_MOUNTS,
            Self::Turn(Initiative::Event) => EVENT_TURN_MOUNTS,
            Self::Cue => CUE_MOUNTS,
        })
    }

    /// `database`, narrowed to what a script of this role may write: all of
    /// it for a turn the user started, only notifications otherwise.
    pub fn database(self, database: &GuardedDatabaseContext) -> GuardedDatabaseContext {
        match self {
            Self::Turn(Initiative::User) => database.clone(),
            Self::Turn(Initiative::Event) | Self::Cue => {
                database.narrowed(&[WriteArea::Notifications])
            }
        }
    }
}
