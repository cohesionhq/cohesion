//! [`Command`], the registry of every request a client can send and what it
//! returns.
//!
//! Request types are plain structs (unit structs when they take no
//! arguments), grouped by domain in the `*_requests` modules. [`Command`] and
//! [`ReplyData`](super::ReplyData) are plain enums, and their glue is
//! derived: `#[derive(Commands)]` reads each variant's `#[reply(...)]` to
//! implement [`Call`](super::Call), `From<Request> for Command` and
//! [`Command::dispatch`]; `#[derive(Replies)]` converts each payload type to
//! and from its `ReplyData` variant. A variant's `via` kind (default
//! [`Database`](super::Database)) picks the [`Route`](super::Route) impl that
//! dispatch uses to run it.
//!
//! Adding a command means a request struct in a `*_requests` module, a
//! `Command` variant here with `#[reply(...)]`, and a `ReplyData` variant if
//! the payload type is new; the compiler points at whatever else is missing
//! (such as the database command that carries it out).

use cohesion_macros::Commands;
use serde::{Deserialize, Serialize};

use super::activity_requests::*;
use super::actor_requests::*;
use super::agent_requests::*;
use super::article_requests::*;
use super::client_requests::*;
use super::connector_requests::*;
use super::contact_requests::*;
use super::document_requests::*;
use super::extension_requests::*;
use super::feed_requests::*;
use super::intelligence_requests::*;
use super::mermaid_requests::{RenderMermaidRequest, RenderedMermaid};
use super::profile_requests::*;
use super::rewind_requests::*;
use super::route_kinds::{FromClient, Intelligence, Local, Script};
use super::script_requests::*;
use super::security_requests::*;
use super::server_requests::*;
use super::storage_requests::*;
use super::tag_requests::*;
use super::task_requests::*;
use super::workspace_requests::*;
use crate::model::{
    Activity, Actor, ActorSession, ActorSessionMessage, ActorSessionSearchHit, Article, Attachment,
    CatalogModel, Client, Contact, ContactMethod, DeletedItem, Document, EndpointModel,
    ExtensionHook, ExtensionPermission, Feed, FeedSummary, IntelligenceAgent, IntelligenceAgentCue,
    IntelligenceAgentMessage, IntelligenceAgentRun, IntelligenceAgentTeam, IntelligenceCostEvent,
    IntelligenceDefault, IntelligenceSpend, Memory, OrganizationUnit, Profile, Resource,
    RestoredItem, SavedTaskView, SearchResult, SecuritySettings, StorageSettings, Tag, Task,
    TaskFolder, Version, VersionSummary, Workspace, WorkspaceResourceKind,
};

/// Every request a client can send; the `args` of a [`Request`](super::Request).
#[derive(Serialize, Deserialize, Debug, Clone, Commands)]
#[serde(tag = "type", content = "args", rename_all = "camelCase")]
pub enum Command {
    #[reply(AboutResponse, request = AboutRequest, via = Local)]
    About,
    /// Answers the web UI this cohesiond was built with, for a client to serve.
    #[reply(WebBundle, request = WebBundleRequest, via = Local)]
    WebBundle,
    /// Answers the hash of the web UI this cohesiond was built with, so a
    /// client can check its copy without fetching the files.
    #[reply(WebBundleHash, request = WebBundleHashRequest, via = Local)]
    WebBundleHash,
    /// Opens a pairing window and answers the setup code a new client
    /// registers with. It works for one registration, for ten minutes.
    /// Refused to a client the security settings don't let open pairing.
    #[reply(PairingCode, request = BeginPairingRequest, via = FromClient)]
    BeginPairing,
    /// Answers whether the client asking may open pairing now, and why not.
    #[reply(PairingAccess, request = GetPairingAccessRequest, via = FromClient)]
    GetPairingAccess,
    /// Evaluates a script on the server.
    #[reply(ScriptResult, via = Script)]
    RunScript(RunScriptRequest),
    /// Draws a Mermaid diagram, or says why it can't be drawn.
    #[reply(RenderedMermaid, via = Local)]
    RenderMermaid(RenderMermaidRequest),
    /// Lists every workspace that is not deleted.
    #[reply(Vec<Workspace>, request = ListWorkspacesRequest)]
    ListWorkspaces,
    /// Creates a workspace with `title`.
    #[reply(Workspace)]
    CreateWorkspace(CreateWorkspaceRequest),
    /// Deletes workspace `id`.
    #[reply(Workspace)]
    DeleteWorkspace(DeleteWorkspaceRequest),
    /// Retitles workspace `id`.
    #[reply(Workspace)]
    RenameWorkspace(RenameWorkspaceRequest),
    /// Sets or clears a workspace's emoji.
    #[reply(Workspace)]
    SetWorkspaceEmoji(SetWorkspaceEmojiRequest),
    /// Fetches the journal for a date in a workspace, if it exists.
    #[reply(Option<Document>)]
    GetJournal(GetJournalRequest),
    /// Appends `content` to the journal for a date in a workspace.
    #[reply(Document)]
    AppendToJournal(AppendToJournalRequest),
    /// Replaces the contents of the journal for a date in a workspace.
    #[reply(Document)]
    EditJournal(EditJournalRequest),
    /// Search documents
    #[reply(Vec<Document>)]
    SearchDocuments(SearchDocumentRequest),
    /// Lists the live documents in a workspace, most recently modified first.
    #[reply(Vec<Document>)]
    ListDocuments(ListDocumentsRequest),
    /// Fetches live document `document_id` in a workspace.
    #[reply(Document)]
    GetDocument(GetDocumentRequest),
    /// Replaces the contents of live document `document_id` in a workspace.
    #[reply(Document)]
    EditDocument(EditDocumentRequest),
    /// Creates an ordinary document in a workspace.
    #[reply(Document)]
    CreateDocument(CreateDocumentRequest),
    /// Retitles live document `document_id` in a workspace.
    #[reply(Document)]
    RenameDocument(RenameDocumentRequest),
    /// Deletes document `document_id` in a workspace.
    #[reply(Document)]
    DeleteDocument(DeleteDocumentRequest),
    /// Lists an item's kept versions, newest first, without their text.
    #[reply(Vec<VersionSummary>)]
    ListVersions(ListVersionsRequest),
    /// Fetches one kept version.
    #[reply(Version)]
    GetVersion(GetVersionRequest),
    /// Puts an item back to a kept version, bringing it back if it was
    /// deleted, and keeps the result as a new version.
    #[reply(RestoredItem)]
    RestoreVersion(RestoreVersionRequest),
    /// Lists the deleted items Rewind can still bring back.
    #[reply(Vec<DeletedItem>)]
    ListDeletedItems(ListDeletedItemsRequest),
    /// Lists task folders
    #[reply(Vec<TaskFolder>)]
    ListTaskFolders(ListTaskFoldersRequest),
    #[reply(TaskFolder)]
    CreateTaskFolder(CreateTaskFolderRequest),
    #[reply(TaskFolder)]
    DeleteTaskFolder(DeleteTaskFolderRequest),
    /// Retitles task folder `folder_id` in a workspace.
    #[reply(TaskFolder)]
    RenameTaskFolder(RenameTaskFolderRequest),
    /// Reorders a workspace's task folders; replies with them in their new order.
    #[reply(Vec<TaskFolder>)]
    ReorderTaskFolders(ReorderTaskFoldersRequest),
    /// Lists a workspace's saved task views, in their order.
    #[reply(Vec<SavedTaskView>)]
    ListSavedTaskViews(ListSavedTaskViewsRequest),
    /// Saves a way of looking at a workspace's tasks under a name.
    #[reply(SavedTaskView)]
    CreateSavedTaskView(CreateSavedTaskViewRequest),
    /// Replaces what saved task view `view_id` lists, and in what order.
    #[reply(SavedTaskView)]
    UpdateSavedTaskView(UpdateSavedTaskViewRequest),
    /// Renames saved task view `view_id` in a workspace.
    #[reply(SavedTaskView)]
    RenameSavedTaskView(RenameSavedTaskViewRequest),
    /// Reorders a workspace's saved task views; replies with them in their new order.
    #[reply(Vec<SavedTaskView>)]
    ReorderSavedTaskViews(ReorderSavedTaskViewsRequest),
    /// Deletes saved task view `view_id` in a workspace.
    #[reply(SavedTaskView)]
    DeleteSavedTaskView(DeleteSavedTaskViewRequest),
    /// Lists a workspace's tags, in their order.
    #[reply(Vec<Tag>)]
    ListTags(ListTagsRequest),
    /// Creates a tag in a workspace.
    #[reply(Tag)]
    CreateTag(CreateTagRequest),
    /// Renames tag `tag_id` in a workspace.
    #[reply(Tag)]
    RenameTag(RenameTagRequest),
    /// Reorders a workspace's tags; replies with them in their new order.
    #[reply(Vec<Tag>)]
    ReorderTags(ReorderTagsRequest),
    /// Deletes tag `tag_id` in a workspace, taking it off everything.
    #[reply(Tag)]
    DeleteTag(DeleteTagRequest),
    /// Puts a tag on a task or takes it off; replies with the task.
    #[reply(Task)]
    SetTagged(SetTaggedRequest),
    /// Lists a workspace's live contacts, by last name, then first name.
    #[reply(Vec<Contact>)]
    ListContacts(ListContactsRequest),
    /// Fetches live contact `contact_id` in a workspace.
    #[reply(Contact)]
    GetContact(GetContactRequest),
    /// Creates a contact in a workspace.
    #[reply(Contact)]
    CreateContact(CreateContactRequest),
    /// Replaces contact `contact_id`'s names and notes.
    #[reply(Contact)]
    UpdateContact(UpdateContactRequest),
    /// Deletes contact `contact_id` in a workspace.
    #[reply(Contact)]
    DeleteContact(DeleteContactRequest),
    /// Adds an email address or phone number to contact `contact_id`.
    #[reply(ContactMethod)]
    AddContactMethod(AddContactMethodRequest),
    /// Replaces contact method `method_id`'s kind and value.
    #[reply(ContactMethod)]
    UpdateContactMethod(UpdateContactMethodRequest),
    /// Removes contact method `method_id` from its contact.
    #[reply(ContactMethod)]
    DeleteContactMethod(DeleteContactMethodRequest),
    /// Sets or removes contact `contact_id`'s photo.
    #[reply(Contact)]
    SetContactPhoto(SetContactPhotoRequest),
    /// Lists a page of the live tasks the filters cover.
    #[reply(TaskPage)]
    ListTasks(ListTasksRequest),
    #[reply(Task)]
    CreateTask(CreateTaskRequest),
    /// Retitles task `task_id` in a workspace.
    #[reply(Task)]
    RenameTask(RenameTaskRequest),
    /// Sets the priority of task `task_id` in a workspace.
    #[reply(Task)]
    SetTaskPriority(SetTaskPriorityRequest),
    /// Sets the size of task `task_id` in a workspace; `null` clears it.
    #[reply(Task)]
    SetTaskSize(SetTaskSizeRequest),
    /// Assigns task `task_id` in a workspace to the user or one of their
    /// agents; `null` leaves it unassigned.
    #[reply(Task)]
    SetTaskAssignee(SetTaskAssigneeRequest),
    /// Replaces the description of task `task_id` in a workspace; `""` clears it.
    #[reply(Task)]
    EditTaskDescription(EditTaskDescriptionRequest),
    /// Sets both dates of task `task_id` in a workspace; `null` clears one.
    #[reply(Task)]
    ScheduleTask(ScheduleTaskRequest),
    /// Moves task `task_id` into folder `folder_id`, or the inbox when absent.
    #[reply(Task)]
    MoveTask(MoveTaskRequest),
    /// Deletes task `task_id` in a workspace.
    #[reply(Task)]
    DeleteTask(DeleteTaskRequest),
    #[reply(Vec<SearchResult>)]
    SearchWorkspace(SearchWorkspaceRequest),
    #[reply(DocumentationResponse, request = DocumentationRequest, via = Script)]
    Documentation,
    #[reply(AcceptedResponse, request = UpdateModelCatalogRequest, via = Intelligence)]
    UpdateModelCatalog,
    /// Lists every intelligence endpoint.
    #[reply(Vec<RedactedIntelligenceEndpoint>, request = ListIntelligenceEndpointsRequest)]
    ListIntelligenceEndpoints,
    #[reply(RedactedIntelligenceEndpoint)]
    CreateIntelligenceEndpoint(CreateIntelligenceEndpointRequest),
    #[reply(RedactedIntelligenceEndpoint)]
    DeleteIntelligenceEndpoint(DeleteIntelligenceEndpointRequest),
    /// Sets or removes where a person sees what an endpoint has used.
    #[reply(RedactedIntelligenceEndpoint)]
    SetIntelligenceEndpointUsageUrl(SetIntelligenceEndpointUsageUrlRequest),
    /// Sets how long an endpoint's server keeps a prompt cached.
    #[reply(RedactedIntelligenceEndpoint)]
    SetIntelligenceEndpointCacheMinutes(SetIntelligenceEndpointCacheMinutesRequest),
    /// Sets or removes the longer prompt cache an endpoint's server offers.
    #[reply(RedactedIntelligenceEndpoint)]
    SetIntelligenceEndpointLongCache(SetIntelligenceEndpointLongCacheRequest),
    /// Imports a model into an endpoint, from the catalog when it lists it.
    #[reply(EndpointModel)]
    AddEndpointModel(AddEndpointModelRequest),
    /// Removes an endpoint's model, unsetting any tier that uses it.
    #[reply(EndpointModel)]
    RemoveEndpointModel(RemoveEndpointModelRequest),
    /// Lists the models an endpoint offers.
    #[reply(Vec<EndpointModel>)]
    ListEndpointModels(ListEndpointModelsRequest),
    /// Makes an endpoint model the default for a tier.
    #[reply(AcceptedResponse)]
    SetIntelligenceDefault(SetIntelligenceDefaultRequest),
    #[reply(Vec<IntelligenceDefault>, request = ListIntelligenceDefaultsRequest)]
    ListIntelligenceDefaults,
    /// Removes the default model of `tier`.
    #[reply(AcceptedResponse)]
    ClearIntelligenceDefault(ClearIntelligenceDefaultRequest),
    /// Lists what model calls cost over each spend period, against its limit.
    #[reply(Vec<IntelligenceSpend>, request = ListIntelligenceSpendRequest)]
    ListIntelligenceSpend,
    /// Sets the spend limit of a period, answering every period's spend.
    #[reply(Vec<IntelligenceSpend>)]
    SetIntelligenceSpendLimit(SetIntelligenceSpendLimitRequest),
    /// Removes the spend limit of a period, answering every period's spend.
    #[reply(Vec<IntelligenceSpend>)]
    ClearIntelligenceSpendLimit(ClearIntelligenceSpendLimitRequest),
    #[reply(Vec<CatalogModel>)]
    ListCatalogModels(ListCatalogModelsRequest),
    /// Fetches intelligence thread `thread_id` with its messages.
    #[reply(IntelligenceThreadDetail)]
    GetIntelligenceThread(GetIntelligenceThreadRequest),
    /// Lists the `limit` most recent intelligence cost events, newest first.
    #[reply(Vec<IntelligenceCostEvent>)]
    ListIntelligenceCostEvents(ListIntelligenceCostEventsRequest),
    #[reply(Vec<Task>)]
    ChangeTasksStatus(ChangeTasksStatusRequest),
    #[reply(AcceptedResponse, via = Local)]
    RegisterExtension(RegisterExtensionRequest),
    /// Removes extension `extension_id`, stopping its sandbox and timers and
    /// dropping its permission and hook grants.
    #[reply(AcceptedResponse, via = Local)]
    UninstallExtension(UninstallExtensionRequest),
    /// Lists every installed extension, in the order its hooks run.
    #[reply(Vec<ExtensionSummary>, request = ListExtensionsRequest)]
    ListExtensions,
    /// Lists the permissions recorded for extension `extension_id`.
    #[reply(Vec<ExtensionPermission>)]
    ListExtensionPermissions(ListExtensionPermissionsRequest),
    /// Enables or disables `permission` for extension `extension_id`,
    /// restarting its sandbox so the change applies at once.
    #[reply(AcceptedResponse, via = Local)]
    SetExtensionPermission(SetExtensionPermissionRequest),
    /// Lists the hooks recorded for extension `extension_id`.
    #[reply(Vec<ExtensionHook>)]
    ListExtensionHooks(ListExtensionHooksRequest),
    /// Enables or disables `hook` for extension `extension_id`, restarting
    /// its sandbox; enabling `launch` calls `onLaunch`.
    #[reply(AcceptedResponse, via = Local)]
    SetExtensionHook(SetExtensionHookRequest),
    /// Asks every extension granted `registry` for the extensions it offers
    /// matching `query`, one section per registry.
    #[reply(Vec<RegistrySection>, via = Local)]
    SearchExtensionRegistries(SearchExtensionRegistriesRequest),
    /// Lists the updates the last check found for installed extensions: one
    /// per extension whose version a registry lists differently.
    #[reply(Vec<ExtensionUpdate>, request = ListExtensionUpdatesRequest, via = Local)]
    ListExtensionUpdates,
    /// Asks every registry about the installed extensions now, and lists the
    /// updates found.
    #[reply(Vec<ExtensionUpdate>, request = CheckExtensionUpdatesRequest, via = Local)]
    CheckExtensionUpdates,
    /// Downloads an extension script and reads its manifest, for review
    /// before installing it.
    #[reply(ExtensionPreview, via = Local)]
    PreviewExtension(PreviewExtensionRequest),
    /// Reads the manifest of an extension script, for review before
    /// installing it.
    #[reply(ExtensionPreview, via = Local)]
    PreviewExtensionScript(PreviewExtensionScriptRequest),
    /// Calls extension `extension_id`'s `serviceRequest(name, args, caller)`,
    /// which needs its `service` permission, telling it which client asks,
    /// and answers what it returns.
    #[reply(ExtensionCallResult, via = FromClient)]
    CallExtension(CallExtensionRequest),
    /// Lists every client that is not deleted, each saying whether it is
    /// connected now and whether it is the one asking.
    #[reply(Vec<ListedClient>, request = ListClientsRequest, via = FromClient)]
    ListClients,
    /// Revokes client `id`; its certificate is no longer trusted, and its
    /// session no longer serves its actors.
    #[reply(Client, via = Local)]
    DeleteClient(DeleteClientRequest),
    /// Renames client `id`; its name is only how it is listed.
    #[reply(Client)]
    RenameClient(RenameClientRequest),
    /// Revokes every client but the one sending this, and answers them.
    #[reply(Vec<Client>, request = RevokeOtherClientsRequest, via = FromClient)]
    RevokeOtherClients,
    /// Answers the security settings.
    #[reply(SecuritySettings, request = GetSecuritySettingsRequest)]
    GetSecuritySettings,
    /// Replaces the security settings.
    #[reply(SecuritySettings)]
    UpdateSecuritySettings(UpdateSecuritySettingsRequest),
    /// Lists a page of the security activity, newest first.
    #[reply(SecurityEventPage)]
    ListSecurityEvents(ListSecurityEventsRequest),
    /// Answers the storage settings: how long the daemon keeps model
    /// threads, actor sessions and security activity, and how many memories
    /// each scope keeps.
    #[reply(StorageSettings, request = GetStorageSettingsRequest)]
    GetStorageSettings,
    /// Replaces the storage settings.
    #[reply(StorageSettings)]
    UpdateStorageSettings(UpdateStorageSettingsRequest),
    /// Answers how large the database file is.
    #[reply(DatabaseSize, request = GetDatabaseSizeRequest)]
    GetDatabaseSize,
    /// Writes a backup of the database to a new file on the daemon's computer.
    #[reply(DatabaseBackup, request = BackupDatabaseRequest, via = Local)]
    BackupDatabase,
    /// Copies the whole database to a new file on the daemon's computer, for
    /// another daemon to import; read it with `ReadDatabaseExport`.
    #[reply(DatabaseExport, request = ExportDatabaseRequest, via = Local)]
    ExportDatabase,
    /// Answers part of an export, removing it once the part reaches the end.
    #[reply(DatabaseExportPart, via = Local)]
    ReadDatabaseExport(ReadDatabaseExportRequest),
    /// Starts an import, which `WriteDatabaseImport` fills.
    #[reply(DatabaseImport, request = BeginDatabaseImportRequest, via = Local)]
    BeginDatabaseImport,
    /// Adds part of the file to an import.
    #[reply(AcceptedResponse, via = Local)]
    WriteDatabaseImport(WriteDatabaseImportRequest),
    /// Replaces everything in the database with an import, keeping this
    /// server's key and clients, then restarts the daemon.
    #[reply(AcceptedResponse, via = Local)]
    FinishDatabaseImport(FinishDatabaseImportRequest),
    /// Answers the user's profile.
    #[reply(Profile, request = GetProfileRequest)]
    GetProfile,
    /// Sets or removes the user's name.
    #[reply(Profile)]
    SetProfileName(SetProfileNameRequest),
    /// Sets the user's bio, or removes it when it is `null` or empty.
    #[reply(Profile)]
    SetProfileBio(SetProfileBioRequest),
    /// Sets or removes the user's photo.
    #[reply(Profile)]
    SetProfilePhoto(SetProfilePhotoRequest),
    /// Creates an agent.
    #[reply(IntelligenceAgent)]
    CreateIntelligenceAgent(CreateIntelligenceAgentRequest),
    /// Lists every agent: the pinned ones in their order, then the rest by
    /// latest message, newest first.
    #[reply(Vec<IntelligenceAgent>, request = ListIntelligenceAgentsRequest)]
    ListIntelligenceAgents,
    /// Reorders the pinned agents; replies with every agent in the new order.
    #[reply(Vec<IntelligenceAgent>)]
    ReorderIntelligenceAgents(ReorderIntelligenceAgentsRequest),
    /// Pins or unpins agent `agent_id`.
    #[reply(IntelligenceAgent)]
    SetAgentPinned(SetAgentPinnedRequest),
    /// Puts agent `agent_id` on a team, or on none.
    #[reply(IntelligenceAgent)]
    SetAgentTeam(SetAgentTeamRequest),
    /// Lists every team of agents, by title.
    #[reply(Vec<IntelligenceAgentTeam>, request = ListAgentTeamsRequest)]
    ListAgentTeams,
    /// Creates a team of agents.
    #[reply(IntelligenceAgentTeam)]
    CreateAgentTeam(CreateAgentTeamRequest),
    /// Renames team `team_id`.
    #[reply(IntelligenceAgentTeam)]
    RenameAgentTeam(RenameAgentTeamRequest),
    /// Deletes team `team_id`; its agents stay, on no team, and its
    /// memories are forgotten.
    #[reply(IntelligenceAgentTeam)]
    DeleteAgentTeam(DeleteAgentTeamRequest),
    /// Fetches agent `agent_id`.
    #[reply(IntelligenceAgent)]
    GetIntelligenceAgent(GetIntelligenceAgentRequest),
    /// Renames, recolors, sets the emoji of or changes the model of agent `agent_id`.
    #[reply(IntelligenceAgent, via = Intelligence)]
    UpdateIntelligenceAgent(UpdateIntelligenceAgentRequest),
    /// Deletes agent `agent_id` with its cues, runs and messages.
    #[reply(IntelligenceAgent, via = Intelligence)]
    DeleteIntelligenceAgent(DeleteIntelligenceAgentRequest),
    /// Marks agent `agent_id`'s conversation read.
    #[reply(IntelligenceAgent)]
    MarkAgentRead(MarkAgentReadRequest),
    /// Pages through an agent's messages, oldest first.
    #[reply(Vec<IntelligenceAgentMessage>)]
    ListAgentMessages(ListAgentMessagesRequest),
    /// Fetches agent message `message_id`.
    #[reply(IntelligenceAgentMessage)]
    GetAgentMessage(GetAgentMessageRequest),
    /// Reacts to an agent's message with an emoji, or clears the reaction;
    /// a reaction queues the agent's turn on it.
    #[reply(AcceptedResponse, via = Intelligence)]
    SetAgentMessageReaction(SetAgentMessageReactionRequest),
    /// Lists the images attached to a message.
    #[reply(Vec<Attachment>)]
    ListAgentMessageAttachments(ListAgentMessageAttachmentsRequest),
    /// Sends a message to an agent: a running turn takes it in, or it
    /// queues the agent's turn on it.
    #[reply(AcceptedResponse, via = Intelligence)]
    SendAgentMessage(SendAgentMessageRequest),
    /// Stops the running turn of an agent's conversation.
    #[reply(AcceptedResponse, via = Intelligence)]
    StopAgentTurn(StopAgentTurnRequest),
    /// Picks an option of an agent's choice, queueing its turn on the pick.
    #[reply(AcceptedResponse, via = Intelligence)]
    RespondToAgentChoice(RespondToAgentChoiceRequest),
    /// Lists agent `agent_id`'s cues, oldest first.
    #[reply(Vec<IntelligenceAgentCue>)]
    ListAgentCues(ListAgentCuesRequest),
    /// Removes cue `cue_id`; an agent left with no cues goes idle.
    #[reply(AcceptedResponse, via = Intelligence)]
    RemoveAgentCue(RemoveAgentCueRequest),
    /// Lists the memories agent `agent_id` sees, its own and the global
    /// ones; only the global ones when absent. Oldest first.
    #[reply(Vec<Memory>)]
    ListMemories(ListMemoriesRequest),
    /// Forgets memory `memory_id`, returning it.
    #[reply(Memory)]
    ForgetMemory(ForgetMemoryRequest),
    /// Lists agent `agent_id`'s most recent background runs, newest first.
    #[reply(Vec<IntelligenceAgentRun>)]
    ListAgentRuns(ListAgentRunsRequest),
    /// Lists the activities still running or waiting, then the `limit`
    /// that ended most recently.
    #[reply(Vec<Activity>)]
    ListActivities(ListActivitiesRequest),
    /// Lists the actors the clients offer.
    #[reply(Vec<Actor>, request = ListActorsRequest)]
    ListActors,
    /// Lists how much of its limits each actor whose client is connected
    /// has used.
    #[reply(Vec<ListedActorUsage>, request = ListActorUsageRequest, via = Local)]
    ListActorUsage,
    /// Lists actor sessions, latest status change first.
    #[reply(Vec<ActorSession>)]
    ListActorSessions(ListActorSessionsRequest),
    #[reply(ActorSession)]
    GetActorSession(GetActorSessionRequest),
    /// Full-text matches among actor sessions' titles and transcripts, best
    /// first.
    #[reply(Vec<ActorSessionSearchHit>)]
    SearchActorSessions(SearchActorSessionsRequest),
    /// Marks actor session `session_id` read.
    #[reply(ActorSession)]
    MarkActorSessionRead(MarkActorSessionReadRequest),
    /// Lists what was said in an actor session, oldest first.
    #[reply(Vec<ActorSessionMessage>)]
    ListActorSessionMessages(ListActorSessionMessagesRequest),
    /// Lists the images attached to an actor session message.
    #[reply(Vec<Attachment>)]
    ListActorSessionMessageAttachments(ListActorSessionMessageAttachmentsRequest),
    /// Hands an actor a new session, through its client.
    #[reply(ActorSession, via = Local)]
    StartActorSession(StartActorSessionRequest),
    /// Says something in an actor session, through its client.
    #[reply(ActorSessionMessage, via = Local)]
    SendActorSessionMessage(SendActorSessionMessageRequest),
    /// Stops what an actor session is doing, leaving it idle.
    #[reply(ActorSession, via = Local)]
    StopActorSession(StopActorSessionRequest),
    /// Resolves an actor session, stopping it first if it is going on.
    #[reply(ActorSession, via = Local)]
    ResolveActorSession(ResolveActorSessionRequest),
    /// Switches an actor session's mode, through its client.
    #[reply(ActorSession, via = Local)]
    SetActorSessionMode(SetActorSessionModeRequest),
    /// Lists every service a `connector`-granted extension reaches.
    #[reply(Vec<ConnectorService>, request = ListConnectorServicesRequest, via = Local)]
    ListConnectorServices,
    /// Asks connector extension `extension_id` for its sign-in page.
    #[reply(ConnectorAuthentication, via = Local)]
    BeginConnectorAuthentication(BeginConnectorAuthenticationRequest),
    /// Adds (or re-authenticates) the account a sign-in `code` belongs to.
    #[reply(RedactedConnectorAccount, via = Local)]
    CompleteConnectorAuthentication(CompleteConnectorAuthenticationRequest),
    /// Lists every connector account.
    #[reply(Vec<RedactedConnectorAccount>, request = ListConnectorAccountsRequest)]
    ListConnectorAccounts,
    /// Removes connector account `account_id` and every resource imported
    /// from it, then tells its extension with `accountRemoved`.
    #[reply(RedactedConnectorAccount, via = Local)]
    DeleteConnectorAccount(DeleteConnectorAccountRequest),
    /// Moves every resource of one connector account to another of the
    /// same connector.
    #[reply(AcceptedResponse)]
    MigrateResources(MigrateResourcesRequest),
    /// Lists the organization units of connector account `account_id`.
    #[reply(Vec<OrganizationUnit>, via = Local)]
    ListConnectorOrganizationUnits(ListConnectorOrganizationUnitsRequest),
    /// Lists the items a connector account offers for import into a workspace.
    #[reply(Vec<ImportableResource>, via = Local)]
    SearchConnectorResources(SearchConnectorResourcesRequest),
    /// Imports one item of a connector account into a workspace as a resource.
    #[reply(Resource, via = Local)]
    ImportResource(ImportResourceRequest),
    /// Lists the resources in a workspace, most recently updated first.
    #[reply(Vec<Resource>)]
    ListResources(ListResourcesRequest),
    /// Fetches resource `resource_id` in a workspace.
    #[reply(Resource)]
    GetResource(GetResourceRequest),
    /// Removes resource `resource_id` from a workspace.
    #[reply(Resource)]
    DeleteResource(DeleteResourceRequest),
    /// Refreshes every resource in a workspace from its service now.
    #[reply(ResourceRefreshSummary, via = Local)]
    RefreshResources(RefreshResourcesRequest),
    /// Lists the kinds of resource shown in a workspace.
    #[reply(Vec<WorkspaceResourceKind>)]
    ListWorkspaceResourceKinds(ListWorkspaceResourceKindsRequest),
    /// Shows or hides a kind of resource in a workspace.
    #[reply(Vec<WorkspaceResourceKind>)]
    SetResourceKindShown(SetResourceKindShownRequest),
    /// Reorders the kinds of resource shown in a workspace.
    #[reply(Vec<WorkspaceResourceKind>)]
    ReorderWorkspaceResourceKinds(ReorderWorkspaceResourceKindsRequest),
    /// Lists a page of the live articles from a feed, or from every feed, most
    /// recently published first, without their contents.
    #[reply(ArticlePage)]
    ListArticles(ListArticlesRequest),
    /// Marks live article `article_id` read or unread.
    #[reply(Article)]
    SetArticleRead(SetArticleReadRequest),
    /// Marks every unread article a listing covers as read.
    #[reply(ArticlesMarkedRead)]
    MarkArticlesRead(MarkArticlesReadRequest),
    /// Lists the feeds in their order, with their unread counts.
    #[reply(Vec<FeedSummary>, request = ListFeedsRequest)]
    ListFeeds,
    /// Reads a feed and follows it.
    #[reply(Feed, via = Local)]
    AddFeed(AddFeedRequest),
    /// Reads live feed `feed_id` now.
    #[reply(Feed, via = Local)]
    RefreshFeed(RefreshFeedRequest),
    /// Reads every live feed now.
    #[reply(Vec<Feed>, request = RefreshFeedsRequest, via = Local)]
    RefreshFeeds,
    /// Retitles live feed `feed_id`.
    #[reply(Feed)]
    RenameFeed(RenameFeedRequest),
    /// Reorders the feeds; replies with them in their new order.
    #[reply(Vec<FeedSummary>)]
    ReorderFeeds(ReorderFeedsRequest),
    /// Sets when live feed `feed_id`'s items are read from their pages.
    #[reply(Feed)]
    SetFeedFullPagePolicy(SetFeedFullPagePolicyRequest),
    /// Stops following feed `feed_id`, removing its items.
    #[reply(Feed)]
    DeleteFeed(DeleteFeedRequest),
    /// Removes feeds' items published before a time.
    #[reply(FeedCleanup)]
    PruneFeeds(PruneFeedsRequest),
    /// A page of the full-text matches among the articles from feeds.
    #[reply(FeedCorpusPage)]
    SearchFeedCorpus(SearchFeedCorpusRequest),
    /// Fetches live article `article_id`, whole.
    #[reply(Article)]
    GetArticle(GetArticleRequest),
    /// Fetches live article `article_id`'s page again.
    #[reply(RefetchedArticle, via = Local)]
    ResnapshotArticle(ResnapshotArticleRequest),
    /// Retitles live article `article_id`.
    #[reply(Article)]
    RenameArticle(RenameArticleRequest),
}
