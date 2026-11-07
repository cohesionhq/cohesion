//! How clients word and order the daemon's kinds: what each permission,
//! hook, priority, size, activity status and agent setting is called, sent in `Hello` so every
//! client shows the same words and none keeps its own copy. A new variant
//! fails to compile here until it is worded.

use cohesion_macros::TsWire;
use serde::{Deserialize, Serialize};

use crate::model::{
    AGENT_REACTIONS, ActivityStatus, ActorSessionStatus, ActorSessionStop, Dialect,
    ExtensionPermissionKind, HookKind, IntelligenceAgentColor, IntelligenceAgentPersonality,
    IntelligenceAgentVerbosity, IntelligenceProvider, IntelligenceTier, PairingOpener,
    SecurityEventKind, SpendPeriod, TaskPriority, TaskSize, VersionChange,
};
use crate::ts_documentation::TsDocumentation;
use crate::ts_wire::{Declarations, TsWire};

/// Every kind a client names, each list in the order clients show it.
#[derive(Serialize, Deserialize, TsWire)]
pub struct Vocabulary {
    /// Extension permissions.
    pub permissions: Vec<Described<ExtensionPermissionKind>>,
    pub hooks: Vec<HookWording>,
    /// Most to least pressing.
    pub task_priorities: Vec<Labeled<TaskPriority>>,
    /// What each kind of version Rewind keeps records.
    pub version_changes: Vec<Labeled<VersionChange>>,
    /// Smallest to largest.
    pub task_sizes: Vec<SizeWording>,
    pub activity_statuses: Vec<Labeled<ActivityStatus>>,
    /// An actor session's statuses: idle, then its activity's.
    pub actor_session_statuses: Vec<Labeled<ActorSessionStatus>>,
    /// How an actor session stopped on its own, as a wake line says it.
    pub actor_session_stops: Vec<Labeled<ActorSessionStop>>,
    pub agent_colors: Vec<Labeled<IntelligenceAgentColor>>,
    pub agent_personalities: Vec<Labeled<IntelligenceAgentPersonality>>,
    pub agent_verbosities: Vec<Labeled<IntelligenceAgentVerbosity>>,
    /// Model tiers, with what each is used for.
    pub tiers: Vec<Described<IntelligenceTier>>,
    /// Spend limit periods, shortest first, with what each counts.
    pub spend_periods: Vec<Described<SpendPeriod>>,
    /// Providers with a built-in endpoint preset.
    pub providers: Vec<ProviderWording>,
    pub dialects: Vec<Labeled<Dialect>>,
    /// The emoji an agent message can be reacted with.
    pub agent_reactions: Vec<String>,
    /// Which clients may open pairing.
    pub pairing_openers: Vec<Labeled<PairingOpener>>,
    /// What each kind of security activity is called.
    pub security_events: Vec<Labeled<SecurityEventKind>>,
}

impl Vocabulary {
    pub fn new() -> Self {
        Self {
            permissions: words(ExtensionPermissionKind::ALL, |kind| Described {
                kind: kind.clone(),
                title: permission_title(kind).to_string(),
                description: permission_description(kind).to_string(),
            }),
            hooks: words(HookKind::ALL, |kind| HookWording {
                kind: kind.clone(),
                extension_clause: extension_clause(kind).to_string(),
                cue_clause: cue_clause(kind).to_string(),
            }),
            task_priorities: TaskPriority::ALL
                .iter()
                .rev()
                .map(|kind| labeled(*kind, priority_label(kind)))
                .collect(),
            version_changes: VersionChange::ALL
                .iter()
                .map(|kind| labeled(*kind, version_change_label(kind)))
                .collect(),
            task_sizes: words(&TaskSize::ALL, |kind| SizeWording {
                kind: *kind,
                label: size_label(*kind).to_string(),
                color: size_color(*kind),
            }),
            activity_statuses: words(ActivityStatus::ALL, |kind| {
                labeled(*kind, activity_status_label(kind))
            }),
            actor_session_statuses: words(ActorSessionStatus::ALL, |kind| {
                labeled(*kind, actor_session_status_label(kind))
            }),
            actor_session_stops: words(ActorSessionStop::ALL, |kind| {
                labeled(*kind, actor_session_stop_label(*kind))
            }),
            agent_colors: words(IntelligenceAgentColor::ALL, |kind| {
                labeled(*kind, color_label(kind))
            }),
            agent_personalities: words(IntelligenceAgentPersonality::ALL, |kind| {
                labeled(*kind, personality_label(kind))
            }),
            agent_verbosities: words(IntelligenceAgentVerbosity::ALL, |kind| {
                labeled(*kind, verbosity_label(kind))
            }),
            tiers: words(IntelligenceTier::ALL, |kind| {
                let (title, description) = tier_words(kind);
                Described {
                    kind: *kind,
                    title: title.to_string(),
                    description: description.to_string(),
                }
            }),
            spend_periods: words(SpendPeriod::ALL, |kind| {
                let (title, description) = spend_period_words(*kind);
                Described {
                    kind: *kind,
                    title: title.to_string(),
                    description: description.to_string(),
                }
            }),
            providers: words(IntelligenceProvider::ALL, |kind| {
                let defaults = kind.defaults();
                ProviderWording {
                    kind: *kind,
                    label: defaults.display_name.to_string(),
                    provider_identifier: defaults.provider_identifier.to_string(),
                }
            }),
            dialects: words(Dialect::ALL, |kind| {
                labeled(kind.clone(), dialect_label(kind))
            }),
            agent_reactions: AGENT_REACTIONS.iter().map(|r| r.to_string()).collect(),
            pairing_openers: words(PairingOpener::ALL, |kind| {
                labeled(*kind, pairing_opener_label(kind))
            }),
            security_events: words(SecurityEventKind::ALL, |kind| {
                labeled(*kind, security_event_label(kind))
            }),
        }
    }
}

/// `Vocabulary` as scripts see it, from `cohesion.vocabulary()`: the same
/// keys as the wire, so a field added above is added here too.
impl TsDocumentation for Vocabulary {
    const TS: &'static str = r#"/**
 * How Cohesion's apps word and order its kinds, as `cohesion.vocabulary()`
 * returns it: plain data, keyed as `Hello` sends it to clients, so a script
 * can name a kind exactly as the apps do. Each list is in the order the
 * apps show it.
 *
 * ```js
 * const words = cohesion.vocabulary();
 * words.activity_statuses.find(w => w.kind === activity.status)?.label; // "Needs you"
 * ```
 */
interface Vocabulary {
  /** Extension permissions. */
  permissions: { kind: ExtensionPermissionKind; title: string; description: string }[];
  /** When each hook fires, as clauses that finish "When …". */
  hooks: { kind: HookKind; extension_clause: string; cue_clause: string }[];
  /** Most to least pressing. */
  task_priorities: { kind: TaskPriority; label: string }[];
  /** What each kind of version Rewind keeps records. */
  version_changes: { kind: VersionChange; label: string }[];
  /** Smallest to largest, each with the color it is drawn in. */
  task_sizes: { kind: TaskSize; label: string; color: IntelligenceAgentColor }[];
  activity_statuses: { kind: ActivityStatus; label: string }[];
  /** An actor session's statuses: idle, then its activity's. */
  actor_session_statuses: {
    kind: "idle" | "running" | "waiting" | "paused" | "succeeded" | "failed" | "cancelled";
    label: string;
  }[];
  /** How an actor session stopped on its own, as a wake line says it. */
  actor_session_stops: { kind: ActorSessionStop; label: string }[];
  agent_colors: { kind: IntelligenceAgentColor; label: string }[];
  agent_personalities: { kind: IntelligenceAgentPersonality; label: string }[];
  agent_verbosities: { kind: IntelligenceAgentVerbosity; label: string }[];
  /** Model tiers, with what each is used for. */
  tiers: { kind: IntelligenceTier; title: string; description: string }[];
  /** Spend limit periods, shortest first, with what each counts. */
  spend_periods: { kind: "hour" | "day" | "month"; title: string; description: string }[];
  /** Providers with a built-in endpoint preset. */
  providers: { kind: IntelligenceProvider; label: string; provider_identifier: string }[];
  dialects: { kind: Dialect; label: string }[];
  /** The emoji an agent message can be reacted with. */
  agent_reactions: string[];
  /** Which clients may open pairing. */
  pairing_openers: { kind: "anyClient" | "localClients"; label: string }[];
  /** What each kind of security activity is called. */
  security_events: { kind: string; label: string }[];
}"#;
    const TS_BRIEF: &'static str = Self::TS;
}

impl Default for Vocabulary {
    fn default() -> Self {
        Self::new()
    }
}

/// A kind with the name a person knows it by.
#[derive(Serialize, Deserialize)]
pub struct Labeled<K> {
    pub kind: K,
    pub label: String,
}

/// A kind with a short title and a one-line description under it.
#[derive(Serialize, Deserialize)]
pub struct Described<K> {
    pub kind: K,
    pub title: String,
    pub description: String,
}

/// When a hook fires, as clauses that finish "When …".
#[derive(Serialize, Deserialize, TsWire)]
pub struct HookWording {
    pub kind: HookKind,
    /// As an extension's hook.
    pub extension_clause: String,
    /// As an agent's cue or run; an agent never hears its own conversation.
    pub cue_clause: String,
}

/// A task size, its name and the color it is drawn in. The colors are a set
/// of their own, none of them the ones priorities use, so a list can be scanned
/// for either.
#[derive(Serialize, Deserialize, TsWire)]
pub struct SizeWording {
    pub kind: TaskSize,
    pub label: String,
    pub color: IntelligenceAgentColor,
}

/// A provider preset, and the catalog provider it reads models from.
#[derive(Serialize, Deserialize, TsWire)]
pub struct ProviderWording {
    pub kind: IntelligenceProvider,
    pub label: String,
    pub provider_identifier: String,
}

impl<K: TsWire> TsWire for Labeled<K> {
    fn ts_type() -> String {
        format!("{{ kind: {}; label: string }}", K::ts_type())
    }

    fn declare(out: &mut Declarations) {
        K::declare(out);
    }
}

impl<K: TsWire> TsWire for Described<K> {
    fn ts_type() -> String {
        format!(
            "{{ kind: {}; title: string; description: string }}",
            K::ts_type()
        )
    }

    fn declare(out: &mut Declarations) {
        K::declare(out);
    }
}

fn words<K, W>(all: &[K], word: impl Fn(&K) -> W) -> Vec<W> {
    all.iter().map(word).collect()
}

fn labeled<K>(kind: K, label: &str) -> Labeled<K> {
    Labeled {
        kind,
        label: label.to_string(),
    }
}

fn permission_title(kind: &ExtensionPermissionKind) -> &'static str {
    use ExtensionPermissionKind::*;
    match kind {
        GlobalReadOnly => "Read your data",
        GlobalReadWrite => "Change your data",
        Network => "Use the network",
        Timer => "Run on a schedule",
        Registry => "Offer extensions to browse",
        OAuth => "Sign in to a service for you",
        Connector => "Import from a service",
        Service => "Run as a service",
        ServiceIsolated => "Run as a service for your apps",
        Services => "Use other extensions’ services",
        Usage => "See model usage",
        Intelligence => "Set up and manage endpoints",
        Notifications => "Send you notifications",
        Actors => "Use actors",
    }
}

fn permission_description(kind: &ExtensionPermissionKind) -> &'static str {
    use ExtensionPermissionKind::*;
    match kind {
        GlobalReadOnly => "See your workspaces, journals, documents and tasks.",
        GlobalReadWrite => "Create, edit and delete what’s in your workspaces.",
        Network => "Send requests to the internet.",
        Timer => "Run code on a repeating timer.",
        Registry => "List extensions for you to browse and install.",
        OAuth => "Connect accounts at a service and keep them signed in.",
        Connector => "Bring an account’s pages and records in as resources.",
        Service => "Answer requests from Cohesion’s apps, the CLI, agents and scripts.",
        ServiceIsolated => "Answer requests from Cohesion’s apps, the web UI and the CLI only.",
        Services => "Call what other installed extensions offer as services.",
        Usage => "See how many tokens Cohesion’s models used and what they cost.",
        Intelligence => "Set up the endpoints and models Cohesion’s agents use.",
        Notifications => "Send notifications that extensions can deliver to your devices.",
        Actors => "Read, dispatch, and interact with actors.",
    }
}

fn pairing_opener_label(kind: &PairingOpener) -> &'static str {
    match kind {
        PairingOpener::AnyClient => "Any client",
        PairingOpener::LocalClients => "Only clients on this computer",
    }
}

fn security_event_label(kind: &SecurityEventKind) -> &'static str {
    use SecurityEventKind::*;
    match kind {
        PairingOpened => "Opened pairing",
        ClientPaired => "Paired",
        ClientRenamed => "Renamed",
        ClientRevoked => "Revoked",
        InactiveClientRevoked => "Revoked for not connecting",
        SettingsChanged => "Changed security settings",
        PairingRefused => "Refused pairing",
    }
}

fn hook_clause(kind: &HookKind) -> &'static str {
    use HookKind::*;
    match kind {
        Launch => "Cohesion starts",
        TaskCreate => "a task is created",
        TaskStatus => "a task’s status changes",
        TaskRename => "a task is renamed",
        TaskMove => "a task moves to another folder",
        TaskPriority => "a task’s priority changes",
        TaskSize => "a task’s size changes",
        TaskAssigned => "a task’s assignee changes",
        TaskDescription => "a task’s description changes",
        TaskStartDate => "a task’s start date changes",
        TaskDueDate => "a task’s due date changes",
        TaskStart => "a task is started",
        TaskDue => "a task is due",
        TaskTagged => "a tag is added to or removed from a task",
        TaskDelete => "a task is deleted",
        TaskFolderCreate => "a task folder is created",
        TaskFolderDelete => "a task folder is deleted",
        WorkspaceCreate => "a workspace is created",
        WorkspaceDelete => "a workspace is deleted",
        JournalSettled => "you finish an edit to a journal entry",
        DocumentSettled => "you finish an edit to a document",
        ResourceImport => "a resource is imported",
        ResourceUpdate => "a refresh brings in a newer copy of a resource",
        ResourceDelete => "a resource is removed",
        FeedArticle => "a feed brings in a new article",
        FeedDelete => "you stop following a feed",
        ArticleRead => "an article is marked read",
        AgentMessage => "a message appears in an agent’s conversation",
        AgentReaction => "someone reacts to a message in an agent’s conversation",
        AgentTurnStart => "an agent starts replying to you",
        AgentTurnEnd => "an agent finishes replying to you",
        ActivityStarted => "an activity starts",
        ActivityUpdated => "an activity’s progress, status or title changes",
        ActivityEnded => "an activity ends",
        ActorMoved => "an actor makes progress",
        DayStart => "the day starts at 6:00 AM",
        DayEnd => "the day ends at 11:00 PM",
        Timer => "a minute begins",
        ExtensionUpdate => "an update is available for an installed extension",
        NotificationSend => "a notification is sent",
        ClientRevoke => "a client is revoked",
    }
}

fn extension_clause(kind: &HookKind) -> &'static str {
    match kind {
        HookKind::Launch => "Cohesion starts, or this extension is updated",
        _ => hook_clause(kind),
    }
}

fn cue_clause(kind: &HookKind) -> &'static str {
    match kind {
        HookKind::AgentMessage => "a message appears in another agent’s conversation",
        HookKind::AgentReaction => "someone reacts to a message in another agent’s conversation",
        _ => hook_clause(kind),
    }
}

fn version_change_label(kind: &VersionChange) -> &'static str {
    match kind {
        VersionChange::Created => "Created",
        VersionChange::Edited => "Edited",
        VersionChange::Deleted => "Deleted",
        VersionChange::Restored => "Restored",
        VersionChange::Earlier => "Earlier version",
    }
}

fn priority_label(kind: &TaskPriority) -> &'static str {
    match kind {
        TaskPriority::Urgent => "Urgent",
        TaskPriority::High => "High",
        TaskPriority::Medium => "Medium",
        TaskPriority::Low => "Low",
        TaskPriority::None => "No priority",
    }
}

fn size_label(kind: TaskSize) -> &'static str {
    const LABELS: [&str; TaskSize::ALL.len()] = ["Tiny", "Small", "Medium", "Large", "Huge"];
    LABELS[usize::from(kind.get()) - 1]
}

fn size_color(kind: TaskSize) -> IntelligenceAgentColor {
    use IntelligenceAgentColor::*;
    const COLORS: [IntelligenceAgentColor; TaskSize::ALL.len()] = [Teal, Green, Blue, Purple, Pink];
    COLORS[usize::from(kind.get()) - 1]
}

fn activity_status_label(kind: &ActivityStatus) -> &'static str {
    match kind {
        ActivityStatus::Running => "Running",
        ActivityStatus::Waiting => "Waiting",
        ActivityStatus::Paused => "Needs you",
        ActivityStatus::Succeeded => "Done",
        ActivityStatus::Failed => "Failed",
        ActivityStatus::Cancelled => "Stopped",
    }
}

/// How an actor session stopped on its own, as a clause after its title.
pub fn actor_session_stop_label(kind: ActorSessionStop) -> &'static str {
    match kind {
        ActorSessionStop::NeedsYou => "needs you",
        ActorSessionStop::Finished => "finished",
        ActorSessionStop::Failed => "failed",
        ActorSessionStop::Interrupted => "was interrupted",
    }
}

fn actor_session_status_label(kind: &ActorSessionStatus) -> &'static str {
    match kind {
        ActorSessionStatus::Working => "Working",
        ActorSessionStatus::NeedsAttention => "Needs you",
        ActorSessionStatus::Idle => "Done",
        ActorSessionStatus::Failed => "Failed",
        ActorSessionStatus::Resolved => "Resolved",
    }
}

fn color_label(kind: &IntelligenceAgentColor) -> &'static str {
    use IntelligenceAgentColor::*;
    match kind {
        Blue => "Blue",
        Green => "Green",
        Orange => "Orange",
        Purple => "Purple",
        Red => "Red",
        Pink => "Pink",
        Teal => "Teal",
        Yellow => "Yellow",
    }
}

fn personality_label(kind: &IntelligenceAgentPersonality) -> &'static str {
    use IntelligenceAgentPersonality::*;
    match kind {
        Balanced => "Balanced",
        Friendly => "Friendly",
        Professional => "Professional",
        Candid => "Candid",
        Playful => "Playful",
    }
}

fn verbosity_label(kind: &IntelligenceAgentVerbosity) -> &'static str {
    use IntelligenceAgentVerbosity::*;
    match kind {
        Terse => "Terse",
        Brief => "Brief",
        Detailed => "Detailed",
        Thorough => "Thorough",
    }
}

fn tier_words(kind: &IntelligenceTier) -> (&'static str, &'static str) {
    match kind {
        IntelligenceTier::Agent => (
            "Agent",
            "Every agent conversation and any script that names no tier.",
        ),
        IntelligenceTier::Low => ("Low", "Cues an agent marks as quick checks."),
        IntelligenceTier::Medium => ("Medium", "Cues an agent marks as ordinary work."),
        IntelligenceTier::High => ("High", "Cues an agent marks as hard reasoning."),
    }
}

fn spend_period_words(kind: SpendPeriod) -> (&'static str, &'static str) {
    match kind {
        SpendPeriod::Hour => ("Per hour", "What model calls cost in the last 60 minutes."),
        SpendPeriod::Day => ("Per day", "What model calls cost in the last 24 hours."),
        SpendPeriod::Month => ("Per month", "What model calls cost in the last 30 days."),
    }
}

fn dialect_label(kind: &Dialect) -> &'static str {
    match kind {
        Dialect::ChatCompletions => "Chat Completions",
        Dialect::Messages => "Messages",
        Dialect::Responses => "Responses",
        Dialect::StatelessResponses => "Responses (stateless)",
    }
}
