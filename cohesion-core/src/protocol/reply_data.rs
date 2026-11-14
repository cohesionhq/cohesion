//! [`ReplyData`], the payload of every successful reply.

use cohesion_macros::Replies;
use serde::{Deserialize, Serialize};

use super::actor_requests::ListedActorUsage;
use super::article_requests::{ArticlePage, ArticlesMarkedRead, RefetchedArticle};
use super::calendar_requests::CalendarSyncSummary;
use super::client_requests::ListedClient;
use super::connector_requests::{
    ConnectorAuthentication, ConnectorService, ImportableResource, RedactedConnectorAccount,
    ResourceRefreshSummary,
};
use super::embed_requests::ResolvedLink;
use super::extension_requests::{
    ExtensionCallResult, ExtensionPreview, ExtensionSummary, ExtensionUpdate, RegistrySection,
};
use super::feed_requests::{FeedCleanup, FeedCorpusPage};
use super::intelligence_requests::{IntelligenceThreadDetail, RedactedIntelligenceEndpoint};
use super::mermaid_requests::RenderedMermaid;
use super::script_requests::{DocumentationResponse, ScriptResult};
use super::security_requests::SecurityEventPage;
use super::server_requests::{
    AboutResponse, AcceptedResponse, PairingAccess, PairingCode, WebBundle, WebBundleHash,
};
use super::storage_requests::{
    DatabaseBackup, DatabaseExport, DatabaseExportPart, DatabaseImport, DatabaseSize,
};
use super::task_requests::TaskPage;
use crate::model::{
    Actor, ActorSession, ActorSessionMessage, ActorSessionSearchHit, AgentAgendaItem, AgentSkill,
    Article, Attachment, Calendar, CalendarEvent, CalendarPerspective, CalendarStore, CatalogModel,
    Client, Component, Contact, ContactMethod, DeletedItem, Document, DocumentFolder,
    DocumentFolderProperty, DocumentSummary, Embed, EndpointModel, ExperimentSetting,
    ExtensionHook, ExtensionPermission, Feed, FeedSummary, IntelligenceAgent, IntelligenceAgentCue,
    IntelligenceAgentMessage, IntelligenceAgentRun, IntelligenceAgentTeam, IntelligenceCostEvent,
    IntelligenceDefault, IntelligenceSpend, Memory, ModuleSetting, OrganizationUnit, Profile,
    Resource, RestoredItem, SavedTaskView, SearchResult, SecuritySettings, StorageSettings,
    TableRow, Tag, Task, TaskFolder, TextSection, Version, VersionSummary, Workspace,
    WorkspaceResourceKind,
};

/// The payload of a successful reply.
#[derive(Serialize, Deserialize, Clone, Replies)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum ReplyData {
    Accepted(AcceptedResponse),
    About(AboutResponse),
    WebBundle(WebBundle),
    WebBundleHash(WebBundleHash),
    PairingCode(PairingCode),
    PairingAccess(PairingAccess),
    Documentation(DocumentationResponse),
    ScriptResult(ScriptResult),
    RenderedMermaid(RenderedMermaid),
    RenderedResource(Option<Component>),
    Workspaces(Vec<Workspace>),
    Workspace(Workspace),
    Document(Document),
    DocumentSummaries(Vec<DocumentSummary>),
    Journal(Option<Document>),
    DocumentFolders(Vec<DocumentFolder>),
    DocumentFolder(DocumentFolder),
    DocumentFolderProperties(Vec<DocumentFolderProperty>),
    DocumentFolderProperty(DocumentFolderProperty),
    TableRows(Vec<TableRow>),
    TableRow(Box<TableRow>),
    Client(Client),
    Clients(Vec<ListedClient>),
    RevokedClients(Vec<Client>),
    SecuritySettings(SecuritySettings),
    SecurityEventPage(SecurityEventPage),
    StorageSettings(StorageSettings),
    Experiments(Vec<ExperimentSetting>),
    Modules(Vec<ModuleSetting>),
    Versions(Vec<VersionSummary>),
    Version(Version),
    DeletedItems(Vec<DeletedItem>),
    RestoredItem(RestoredItem),
    DatabaseSize(DatabaseSize),
    DatabaseBackup(DatabaseBackup),
    DatabaseExport(DatabaseExport),
    DatabaseExportPart(DatabaseExportPart),
    DatabaseImport(DatabaseImport),
    Profile(Profile),
    TaskFolders(Vec<TaskFolder>),
    TaskFolder(TaskFolder),
    SavedTaskViews(Vec<SavedTaskView>),
    SavedTaskView(SavedTaskView),
    Tags(Vec<Tag>),
    Tag(Tag),
    Contacts(Vec<Contact>),
    Contact(Contact),
    ContactMethod(ContactMethod),
    CalendarStores(Vec<CalendarStore>),
    CalendarStore(CalendarStore),
    Calendars(Vec<Calendar>),
    Calendar(Calendar),
    CalendarEvents(Vec<CalendarEvent>),
    CalendarEvent(Box<CalendarEvent>),
    CalendarSyncSummary(CalendarSyncSummary),
    CalendarPerspective(CalendarPerspective),
    Tasks(Vec<Task>),
    Task(Task),
    Results(Vec<SearchResult>),
    IntelligenceEndpoints(Vec<RedactedIntelligenceEndpoint>),
    IntelligenceEndpoint(RedactedIntelligenceEndpoint),
    EndpointModels(Vec<EndpointModel>),
    IntelligenceDefaults(Vec<IntelligenceDefault>),
    CatalogModels(Vec<CatalogModel>),
    IntelligenceThread(IntelligenceThreadDetail),
    IntelligenceCostEvents(Vec<IntelligenceCostEvent>),
    IntelligenceSpend(Vec<IntelligenceSpend>),
    Extensions(Vec<ExtensionSummary>),
    ExtensionPermissions(Vec<ExtensionPermission>),
    ExtensionHooks(Vec<ExtensionHook>),
    RegistrySections(Vec<RegistrySection>),
    ExtensionUpdates(Vec<ExtensionUpdate>),
    ExtensionPreview(ExtensionPreview),
    ExtensionCallResult(ExtensionCallResult),
    IntelligenceAgent(IntelligenceAgent),
    IntelligenceAgents(Vec<IntelligenceAgent>),
    AgentTeam(IntelligenceAgentTeam),
    AgentTeams(Vec<IntelligenceAgentTeam>),
    AgentMessage(Box<IntelligenceAgentMessage>),
    AgentMessages(Vec<IntelligenceAgentMessage>),
    Attachments(Vec<Attachment>),
    AgentCues(Vec<IntelligenceAgentCue>),
    AgentAgenda(Vec<AgentAgendaItem>),
    AgentSkill(AgentSkill),
    AgentSkills(Vec<AgentSkill>),
    AgentRuns(Vec<IntelligenceAgentRun>),
    Actors(Vec<Actor>),
    ActorUsage(Vec<ListedActorUsage>),
    ActorSessions(Vec<ActorSession>),
    ActorSession(Box<ActorSession>),
    ActorSessionMessages(Vec<ActorSessionMessage>),
    ActorSessionMessage(ActorSessionMessage),
    ActorSessionSearchHits(Vec<ActorSessionSearchHit>),
    Memories(Vec<Memory>),
    Memory(Memory),
    ConnectorServices(Vec<ConnectorService>),
    ConnectorAuthentication(ConnectorAuthentication),
    ConnectorAccount(Box<RedactedConnectorAccount>),
    ConnectorAccounts(Vec<RedactedConnectorAccount>),
    OrganizationUnits(Vec<OrganizationUnit>),
    ImportableResources(Vec<ImportableResource>),
    Resource(Resource),
    Resources(Vec<Resource>),
    ResourceRefreshSummary(ResourceRefreshSummary),
    WorkspaceResourceKinds(Vec<WorkspaceResourceKind>),
    // Boxed, as every model with a body or many prices is: it dwarfs the
    // other replies.
    EndpointModel(Box<EndpointModel>),
    Article(Box<Article>),
    Embed(Box<Embed>),
    Embeds(Vec<Embed>),
    ResolvedLinks(Vec<ResolvedLink>),
    TextSections(Vec<TextSection>),
    Feed(Feed),
    Feeds(Vec<Feed>),
    FeedSummaries(Vec<FeedSummary>),
    FeedCleanup(FeedCleanup),
    FeedCorpusPage(FeedCorpusPage),
    RefetchedArticle(Box<RefetchedArticle>),
    ArticlesMarkedRead(ArticlesMarkedRead),
    ArticlePage(ArticlePage),
    TaskPage(TaskPage),
}
