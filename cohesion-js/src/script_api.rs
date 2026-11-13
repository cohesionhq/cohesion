//! The script API, listed once by module in the `script_api!` invocation:
//! every global, entity class, enum and options type scripts can see, the
//! code that puts them on a sandbox, and their TypeScript declarations
//! joined into [`TS_DECLARATIONS`] and grouped into [`MODULES`], which an
//! agent's code tool declares up front or loads one at a time.

use rquickjs::class::JsClass;
use rquickjs::function::Constructor;
use rquickjs::{Ctx, Exception, FromJs, IntoJs, Object, Value};

use cohesion_core::model::{
    AccountDescription, ActorEffort, ActorMode, ActorModel, ActorSessionStatus, BackendAllDayEvent,
    BackendCalendar, BackendEvent, BackendEventFields, BackendEventTiming, BackendInvitee,
    BackendTimedEvent, Block, CalendarColor, CheckboxBlock, Component, ConnectorResource,
    ConnectorResourceFilters, ConnectorResourcePreview, ContactMethodKind, Credential, Dialect,
    DocumentFolderRepresentation, ExtensionManifest, ExtensionPermissionKind, FormulaUpdateMode,
    HeadingBlock, HeadingLevel, HookKind, IntelligenceAgentColor, IntelligenceAgentPersonality,
    IntelligenceAgentVerbosity, IntelligenceEffort, IntelligenceProvider, IntelligenceTier,
    InviteeResponse, LinkBlock, ListSymbol, MetadataKey, NewContactMethod, OrganizationUnit,
    OverviewBlock, PkceChallenge, PropertyDataType, PublishedPrecision, RegistryResult,
    ResourceContentKind, ResourceKind, ResourceKindDescription, SearchEntityKind,
    ServiceDescription, ServiceFunctionManifest, ServiceManifest, TextBlock, VersionAuthor,
    VersionChange, VersionedItem,
};
use cohesion_core::protocol::{PlanUsage, Vocabulary};
use cohesion_core::ts_documentation::TsDocumentation;
use cohesion_core::value::ValueError;
use cohesion_core::{
    DayFilter, DayRange, DocumentType, TaskAssignee, TaskPriority, TaskSize, TaskSort, TaskStatus,
};

use crate::conversion::{FromJsField, ToJs};
use crate::entities::workspace_options::TaskFolderListOptions;
use crate::entities::{
    account, actor, actor_session, agent, agent_team, article, calendar, connector,
    connector_account, connector_account_options, contact, document, document_folder,
    document_folder_property, endpoint_model, feed, intelligence_endpoint, resource, rewind,
    search_result, tag, task, task_folder, usage, workspace, workspace_options,
};
use crate::globals::{
    AccountEndpointOptions, AccountsGlobal, ActorDispatchOptions, ActorSessionMessage,
    ActorsGlobal, AgentCueOptions, AgentGlobal, CalendarCreateOptions, CohesionGlobal,
    ConnectorsGlobal, CorpusSearchOptions, CustomEndpointOptions, EndpointModelOptions,
    EndpointOptions, ImageOptions, ImagesGlobal, IntelligenceGlobal, LoggerGlobal,
    ModelPriceOptions, Mount, NetworkBodyEncoding, NetworkFetchOptions, NetworkGlobal,
    NetworkMethod, NetworkResponse, NotificationsGlobal, ProviderEndpointOptions, ServiceMap,
    ServicesGlobal, StorageGlobal, TimerGlobal, TimerOptions, TimerUnit, UsageGlobal, UsageOptions,
    WorkspaceListOptions,
};
use crate::sandbox::{AgentMessage, Notification, RevokedClient};

/// Defines one entity class on the global object.
pub type Definer = for<'js> fn(&Ctx<'js>, &Object<'js>) -> rquickjs::Result<()>;

script_api! {
    /// What nearly every agent script uses: `cohesion` and the entities it
    /// returns for workspaces, tasks, documents and search, and the types two
    /// modules share.
    CORE = "core", upfront, "`cohesion`: workspaces, tasks and their folders, documents and \
        their folders, and search." {
        globals: [CohesionGlobal],
        entities: [
            workspace::JsWorkspace,
            document::JsDocument,
            document_folder::JsDocumentFolder,
            search_result::JsSearchResult,
            task::JsTask,
            task_folder::JsTaskFolder,
            tag::JsTag,
        ],
        enums: [
            DocumentType,
            DocumentFolderRepresentation,
            IntelligenceAgentColor,
            ListSymbol,
            SearchEntityKind,
            TaskStatus,
            TaskPriority,
            TaskSort,
            PublishedPrecision,
        ],
        options: [
            workspace_options::TaskListOptions,
            workspace_options::TaskCreateOptions,
            workspace_options::DocumentListOptions,
            WorkspaceListOptions,
            workspace_options::TaskFolderCreateOptions,
            TaskFolderListOptions,
        ],
        interfaces: [TaskSize, TaskAssignee, DayFilter, DayRange],
        mounted: [TaskStatus, TaskPriority, TaskSort, DocumentFolderRepresentation],
    }

    /// The `agent` global, for an agent the user is talking to.
    AGENT = "agent", upfront, "`agent`: your cues." {
        globals: [AgentGlobal],
        enums: [IntelligenceEffort],
        mounted: [IntelligenceEffort],
    }

    /// `services.use`, whose `ServiceMap` the installed services' runtime
    /// declarations fill in.
    SERVICES = "services", upfront, "`services.use`: the functions installed services \
        declare." {
        globals: [ServicesGlobal],
        interfaces: [ServiceMap],
    }

    /// The user's agents and their teams, which `cohesion.agents()`,
    /// `cohesion.agent(id)`, `cohesion.teams()` and `cohesion.team(id)` return.
    AGENTS = "agents", on_demand, "`Agent` and `Team`: the user's agents and the teams they are \
        organized into, as `cohesion.agents()` and `cohesion.teams()` return them." {
        entities: [agent::JsAgent, agent_team::JsTeam],
        enums: [
            IntelligenceAgentPersonality,
            IntelligenceAgentVerbosity,
        ],
    }

    /// A table folder's columns and their options, which
    /// `DocumentFolder.properties()` returns.
    TABLES = "tables", on_demand, "Tables: a document folder's columns \
        (`DocumentFolder.properties()`), their types, options and formulas, and each \
        document's values in them (`Document.values()`, `Document.setValue()`)." {
        entities: [
            document_folder_property::JsDocumentFolderProperty,
            document_folder_property::JsPropertyOption,
        ],
        enums: [PropertyDataType, FormulaUpdateMode],
        options: [document_folder_property::PropertyCreateOptions],
        mounted: [PropertyDataType, FormulaUpdateMode],
    }

    /// Articles and feeds, which `cohesion` returns.
    ARTICLES = "articles", on_demand, "Followed feeds, the articles they brought in, and \
        searching them." {
        entities: [article::JsArticle, feed::JsFeed],
        options: [CorpusSearchOptions],
        interfaces: [article::ScriptSnapshot<'static>, article::ScriptFetchFailure<'static>],
    }

    /// Resources, which `Workspace` returns.
    RESOURCES = "resources", on_demand, "Resources: items imported into a workspace from \
        connected services." {
        entities: [resource::JsResource],
        enums: [ResourceContentKind, MetadataKey],
        options: [workspace_options::ResourceListOptions],
        mounted: [MetadataKey],
    }

    /// Rewind's kept versions, which `Task.versions()` and
    /// `Document.versions()` return.
    REWIND = "rewind", on_demand, "Rewind: the kept versions of tasks and documents, as \
        `versions()` returns them, to read and restore." {
        entities: [rewind::JsVersionSummary, rewind::JsVersion],
        enums: [VersionChange, VersionAuthor],
        interfaces: [VersionedItem],
    }

    /// The words `cohesion.vocabulary()` returns.
    VOCABULARY = "vocabulary", on_demand, "`Vocabulary`: how the apps word Cohesion's kinds, as \
        `cohesion.vocabulary()` returns it." {
        interfaces: [Vocabulary],
    }

    /// Contacts, which `Workspace` returns.
    CONTACTS = "contacts", on_demand, "Contacts: the people a workspace keeps, with their \
        email addresses, phone numbers and notes." {
        entities: [contact::JsContact, contact::JsContactMethod],
        enums: [ContactMethodKind],
        options: [workspace_options::ContactCreateOptions, contact::ContactPhotoImage],
        interfaces: [NewContactMethod],
        mounted: [ContactMethodKind],
    }

    /// Calendars, which `cohesion` returns, and the shapes a calendar
    /// backend's functions take and return.
    CALENDARS = "calendars", on_demand, "Calendars: the user's calendar stores, calendars and \
        their events, shared by every workspace, and the Calendar Backend contract a \
        `calendar:backend` extension implements." {
        entities: [
            calendar::JsCalendarStore,
            calendar::JsCalendar,
            calendar::JsCalendarEvent,
        ],
        enums: [InviteeResponse],
        options: [
            calendar::CalendarEventListOptions,
            calendar::CalendarEventOptions,
            CalendarCreateOptions,
        ],
        interfaces: [
            CalendarColor,
            BackendCalendar,
            BackendEvent,
            BackendEventFields,
            BackendEventTiming,
            BackendTimedEvent,
            BackendAllDayEvent,
            BackendInvitee,
        ],
    }

    /// What `agent.cue` takes, the `AgentMessage` an `agent:message` or
    /// `agent:reaction` cue wakes with and the `RevokedClient` a
    /// `client:revoke` one does.
    CUES = "cues", on_demand, "`agent.cue` options: hooks, the `AgentMessage` an \
        `agent:message` or `agent:reaction` cue receives and the `RevokedClient` a \
        `client:revoke` cue receives." {
        enums: [HookKind],
        options: [AgentCueOptions],
        interfaces: [AgentMessage, RevokedClient],
    }

    /// The `actors` global, the actors and sessions it returns, and the
    /// `ActorSession` an `actor:*` cue wakes with.
    ACTORS = "actors", on_demand, "`actors`: the actors on the user's devices (Claude Code in a \
        directory, say): hand them sessions, follow and reply in them." {
        globals: [ActorsGlobal],
        entities: [actor::JsActor, actor_session::JsActorSession],
        enums: [ActorSessionStatus],
        options: [ActorDispatchOptions],
        interfaces: [ActorMode, ActorModel, ActorEffort, PlanUsage, ActorSessionMessage],
    }

    /// The `images` global, for an agent's turns.
    IMAGES = "images", on_demand, "`images`: attach images, made, fetched or from an actor \
        task, to show the user with `send_message`." {
        globals: [ImagesGlobal],
        options: [ImageOptions],
    }

    /// The `storage` global: an extension's or agent's own key/value store.
    STORAGE = "storage", on_demand, "`storage`: your own key/value store, kept across \
        conversations and runs." {
        globals: [StorageGlobal],
    }

    /// `network.fetch`.
    NETWORK = "network", on_demand, "`network.fetch`: HTTP requests to the web." {
        globals: [NetworkGlobal],
        enums: [NetworkMethod, NetworkBodyEncoding],
        options: [NetworkFetchOptions],
        interfaces: [NetworkResponse],
        mounted: [NetworkMethod, NetworkBodyEncoding],
    }

    /// The `connectors` global and what its accounts return.
    CONNECTORS = "connectors", on_demand, "`connectors`: browse the live services behind the \
        accounts under \"Connected services\" and import items from them." {
        globals: [ConnectorsGlobal],
        entities: [connector_account::JsConnectorAccount, connector::JsConnector],
        options: [
            connector_account_options::AccountResourceListOptions,
            connector_account_options::ConnectorItemFilters,
        ],
        interfaces: [
            AccountDescription,
            ResourceKind,
            OrganizationUnit,
            ConnectorResourceFilters,
            ConnectorResourcePreview,
            ConnectorResource,
        ],
    }

    /// The `intelligence` global: endpoints and models.
    INTELLIGENCE = "intelligence", on_demand, "`intelligence`: LLM endpoints and their \
        models." {
        globals: [IntelligenceGlobal],
        entities: [intelligence_endpoint::JsIntelligenceEndpoint, endpoint_model::JsEndpointModel],
        enums: [Dialect, IntelligenceProvider, IntelligenceTier],
        options: [
            EndpointOptions,
            ProviderEndpointOptions,
            CustomEndpointOptions,
            EndpointModelOptions,
            ModelPriceOptions,
        ],
        mounted: [IntelligenceProvider, Dialect, IntelligenceTier],
    }

    /// The `log` global.
    LOG = "log", on_demand, "`log`: lines in the daemon's log." {
        globals: [LoggerGlobal],
    }

    /// The `accounts` global: an extension's signed-in accounts.
    ACCOUNTS = "accounts", on_demand, "`accounts`: an extension's signed-in accounts and their \
        credentials." {
        globals: [AccountsGlobal],
        entities: [account::JsAccount],
        options: [AccountEndpointOptions],
        interfaces: [Credential],
    }

    /// The `timer` global.
    TIMER = "timer", on_demand, "`timer`: recurring calls in an extension." {
        globals: [TimerGlobal],
        enums: [TimerUnit],
        options: [TimerOptions],
        mounted: [TimerUnit],
    }

    /// What an extension script returns to describe itself, and the
    /// CohesionUI interface a connector's `renderResource` returns; no
    /// mount documents it.
    EXTENSIONS = "extensions", on_demand, "An extension's manifest, permissions, connector \
        service, registry results and the CohesionUI interfaces a connector shows resources \
        with." {
        enums: [ExtensionPermissionKind],
        interfaces: [
            ExtensionManifest,
            ServiceManifest,
            ServiceFunctionManifest,
            RegistryResult,
            ServiceDescription,
            PkceChallenge,
            ResourceKindDescription,
            Component,
            Block,
            TextBlock,
            CheckboxBlock,
            OverviewBlock,
            HeadingBlock,
            HeadingLevel,
            LinkBlock,
        ],
    }

    /// The `usage` global: model calls, tokens and cost.
    USAGE = "usage", on_demand, "`usage`: how many tokens Cohesion's models used and what they \
        cost." {
        globals: [UsageGlobal],
        entities: [usage::JsUsage, usage::JsModelUsage],
        options: [UsageOptions],
    }

    /// The `notifications` global, and the `Notification` that
    /// `notification:send` hands its hooks.
    NOTIFICATIONS = "notifications", on_demand, "`notifications`: send the user a notification, \
        which extensions push to their devices." {
        globals: [NotificationsGlobal],
        interfaces: [Notification],
    }
}

/// The script API as the contents of a `.d.ts` file, which editors read for
/// completion in scripts. Joined at compile time: `TS_DOCUMENTATION`, then a
/// trailing newline.
pub const TS_DECLARATIONS: &str = {
    const SEPARATOR: &[u8] = b"\n\n";
    const LEN: usize = {
        let mut len = 1;
        let mut i = 0;
        while i < TS_DOCUMENTATION.len() {
            if i > 0 {
                len += SEPARATOR.len();
            }
            len += TS_DOCUMENTATION[i].len();
            i += 1;
        }
        len
    };
    static BYTES: [u8; LEN] = {
        const fn push(out: &mut [u8; LEN], at: usize, bytes: &[u8]) -> usize {
            let mut i = 0;
            while i < bytes.len() {
                out[at + i] = bytes[i];
                i += 1;
            }
            at + bytes.len()
        }
        let mut out = [0; LEN];
        let mut at = 0;
        let mut i = 0;
        while i < TS_DOCUMENTATION.len() {
            if i > 0 {
                at = push(&mut out, at, SEPARATOR);
            }
            at = push(&mut out, at, TS_DOCUMENTATION[i].as_bytes());
            i += 1;
        }
        push(&mut out, at, b"\n");
        out
    };
    // Joining whole strings on ASCII keeps it valid UTF-8.
    match std::str::from_utf8(&BYTES) {
        Ok(declarations) => declarations,
        Err(_) => unreachable!(),
    }
};

/// A named part of the script API, so an agent's code tool can declare
/// the [`Upfront`](Load::Upfront) modules its sandbox mounts and load the
/// rest only when a script needs them. Which modules a sandbox gets
/// follows from its mounts ([`Mount::modules`]).
pub struct ScriptModule {
    /// The name the module is asked for by, such as `"network"`.
    pub name: &'static str,
    /// One line saying what the module reaches, for an index of modules.
    pub summary: &'static str,
    /// Whether an agent's code tool declares it or loads it on demand.
    pub load: Load,
    /// Its entries of [`TS_DOCUMENTATION`]; types it shares with another
    /// module live in [`CORE`].
    pub declarations: &'static [&'static str],
    /// The same entries with only the first paragraph of each doc comment.
    pub brief: &'static [&'static str],
}

/// When an agent's code tool shows a module.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Load {
    /// Declared in the tool's description, for what nearly every script uses.
    Upfront,
    /// Indexed in the description and loaded through the `documentation` tool.
    OnDemand,
}

impl ScriptModule {
    /// The module's declarations as `.d.ts` source.
    pub fn source(&self) -> String {
        self.declarations.join("\n\n")
    }

    /// [`source`](Self::source) with only the first paragraph of each doc
    /// comment, for a module declared up front.
    pub fn brief_source(&self) -> String {
        self.brief.join("\n\n")
    }
}

/// The modules documenting `mounts` that load as `load`, in mount order.
pub fn modules(mounts: &[Mount], load: Load) -> impl Iterator<Item = &'static ScriptModule> + '_ {
    mounts
        .iter()
        .flat_map(|mount| mount.modules())
        .filter(move |module| module.load == load)
}

/// Defines `E`'s class on `target` as `target[E::NAME]`. Scripts cannot
/// construct entities, so the constructor throws; it still makes `instanceof`
/// and `<Name>.prototype` work.
fn define<'js, E: for<'a> JsClass<'a>>(
    ctx: &Ctx<'js>,
    target: &Object<'js>,
) -> rquickjs::Result<()> {
    let constructor = Constructor::new_class::<E, _, _>(ctx.clone(), |ctx: Ctx<'js>| {
        rquickjs::Result::<Object<'js>>::Err(Exception::throw_type(
            &ctx,
            &format!("{} cannot be constructed from scripts", E::NAME),
        ))
    })?;
    target.set(E::NAME, constructor)
}

#[cfg(test)]
mod tests;
