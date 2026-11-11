// What the mock daemon holds: its rows, as `seed` fills them and the handlers
// change them.

import type {
	Activity,
	Actor,
	ActorSession,
	ActorSessionMessage,
	AgentAgendaItem,
	AgentSkill,
	Article,
	Attachment,
	CatalogModel,
	Client,
	Component,
	Connector,
	Contact,
	ContactMethod,
	DatabaseSize,
	Document,
	DocumentFolder,
	DocumentFolderProperty,
	EndpointModel,
	ExtensionHook,
	ExtensionManifest,
	ExtensionPermission,
	ExtensionSummary,
	Feed,
	IntelligenceAgent,
	IntelligenceAgentCue,
	IntelligenceAgentMessage,
	IntelligenceAgentRun,
	IntelligenceAgentTeam,
	IntelligenceCostEvent,
	IntelligenceDefault,
	IntelligenceThread,
	IntelligenceThreadMessage,
	Memory,
	OrganizationUnit,
	Profile,
	PropertyValue,
	RedactedConnectorAccount,
	RedactedIntelligenceEndpoint,
	RegistryResult,
	Resource,
	ResourceContentKind,
	SavedTaskView,
	SecurityEvent,
	SecuritySettings,
	SpendPeriod,
	StorageSettings,
	Tag,
	Task,
	TaskFolder,
	Version,
	Workspace,
	WorkspaceResourceKind
} from '../types';

/** The daemon's `TranscriptItem` (cohesion-intelligence/src/transcript_item.rs) as serde JSON. */
export type TranscriptItem =
	| { UserMessage: { text: string } }
	| { AssistantMessage: { text: string } }
	| { Thinking: { Standard: { thought: string; signature: string | null; in_content: boolean } } }
	| { ToolCallRequest: { call_identifier: string; tool_name: string; arguments: string } }
	| {
			ToolCallResponse: {
				call_identifier: string;
				response: { Success: string } | { Failure: string };
			};
	  };

/** A script a registry points at: its text and the manifest the daemon reads from it. */
export interface MockSource {
	script: string;
	manifest: ExtensionManifest;
}

/**
 * What an installed registry extension's `registryResults` returns, or the
 * error it throws. Its `registryLookup` answers from the same results.
 */
export interface MockRegistry {
	extension_id: number;
	results: RegistryResult[];
	error: string | null;
}

/** A record a connected service holds, as the connector's `getResource` returns it. */
export interface MockRemoteResource {
	identifier: string;
	/** The organization unit it is filed under. */
	unit: string;
	title: string;
	kind_identifier: string;
	content_kind: ResourceContentKind;
	contents: string;
	last_updated: number;
	/** What the connector's `renderResource` returns for it; none shows the contents. */
	render?: Component;
}

/** What an installed connector extension's functions return, or the error its description throws. */
export interface MockConnector {
	extension_id: number;
	title: string;
	unit: string;
	error: string | null;
	/** The remote account any sign-in code adds. */
	account: { identifier: string; description: string };
	units: OrganizationUnit[];
	/** The kinds its description declares, each a key, a title and a plural. */
	kinds: [string, string, string][];
	resources: MockRemoteResource[];
}

/**
 * An account as the daemon stores it; its connector and whether it can
 * import are added when it is sent.
 */
export type MockConnectorAccount = Omit<RedactedConnectorAccount, 'connector' | 'importable'> & {
	connector_id: number;
};

/** A page as the mock fetched it, like the daemon's `ArticleCapture`. */
export interface MockCapture {
	final_url: string;
	page_title: string | null;
	text: string;
	truncated: boolean;
}

export interface MockState {
	workspaces: Workspace[];
	documents: Document[];
	documentFolders: DocumentFolder[];
	folders: TaskFolder[];
	tasks: Task[];
	endpoints: RedactedIntelligenceEndpoint[];
	/** Flat, as the daemon reads them: an imported row carries its catalog row's values. */
	endpointModels: EndpointModel[];
	defaults: IntelligenceDefault[];
	catalog: CatalogModel[];
	threads: IntelligenceThread[];
	messages: IntelligenceThreadMessage[];
	costEvents: IntelligenceCostEvent[];
	/** One per period at most; a period without one has no limit. */
	spendLimits: { period: SpendPeriod; amount: number }[];
	clients: Client[];
	/** The ids of the clients connected now, which `listClients` marks online. */
	onlineClients: number[];
	/** The client the page is, which `listClients` marks current. */
	currentClient: number;
	security: SecuritySettings;
	/** Oldest first. */
	securityEvents: SecurityEvent[];
	storage: StorageSettings;
	versions: Version[];
	databaseSize: DatabaseSize;
	profile: Profile;
	extensions: ExtensionSummary[];
	permissions: ExtensionPermission[];
	hooks: ExtensionHook[];
	/** Scripts by URL, for `previewExtension` and installing what was previewed. */
	sources: Map<string, MockSource>;
	registries: MockRegistry[];
	agents: IntelligenceAgent[];
	agentTeams: IntelligenceAgentTeam[];
	agentMessages: IntelligenceAgentMessage[];
	agentAttachments: Attachment[];
	agentCues: IntelligenceAgentCue[];
	agentAgenda: AgentAgendaItem[];
	agentRuns: IntelligenceAgentRun[];
	activities: Activity[];
	actors: Actor[];
	actorSessions: ActorSession[];
	actorSessionMessages: ActorSessionMessage[];
	actorSessionAttachments: Attachment[];
	memories: Memory[];
	agentSkills: AgentSkill[];
	connectors: MockConnector[];
	/** The stored definitions, as the `connectors` table holds them. */
	definitions: Connector[];
	connectorAccounts: MockConnectorAccount[];
	resources: Resource[];
	/** The kinds shown in each workspace. */
	shownKinds: WorkspaceResourceKind[];
	articles: Article[];
	feeds: Feed[];
	contacts: Omit<Contact, 'methods'>[];
	/** A contact method's row, which has no workspace of its own. */
	contactMethods: MockContactMethod[];
	tags: Tag[];
	/** Which tag is on which task. */
	tagged: { tag_id: number; task_id: number }[];
	savedTaskViews: SavedTaskView[];
	/** Every folder's; `sort_order` orders one folder's. */
	folderProperties: DocumentFolderProperty[];
	/** One per document and property holding a value, as `document_values` holds them. */
	documentValues: MockDocumentValue[];
}

export interface MockDocumentValue {
	document_id: number;
	property_id: number;
	value: PropertyValue;
}

export type MockContactMethod = Omit<ContactMethod, 'workspace_id'>;

/** A contact as the mock keeps it; its methods are rows of their own. */
export type MockContact = Omit<Contact, 'methods'>;
