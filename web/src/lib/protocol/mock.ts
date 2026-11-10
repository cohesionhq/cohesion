// An in-memory daemon implementing every request kind, for `VITE_MOCK=1` and
// as the fixture for tests. Each `createMock()` call starts from fresh seed data.

import { parseItemLink, textPieces } from '$lib/format/itemLinks';
import { ProtocolError, toProtocolError } from './errors';
import type {
	DayFilter,
	Embed,
	Activity,
	SecurityEvent,
	SecurityEventKind,
	SecuritySettings,
	Profile,
	StorageSettings,
	DatabaseSize,
	Actor,
	ActorMessageContent,
	ActorSession,
	ActorSessionMessage,
	ActorSessionMessageContent,
	ActorSessionSearchHit,
	Connector,
	AgentChoice,
	AgentCodeExecution,
	AgentMemoryChange,
	AgentMessageContent,
	AgentRunPart,
	Component,
	ContextRebuildReason,
	ArgsOf,
	Resource,
	ResourceContentKind,
	Dialect,
	CatalogModel,
	Client,
	Contact,
	ContactMethod,
	ContactMethodKind,
	Photo,
	Document,
	DocumentFolder,
	EndpointModel,
	ExtensionHook,
	ExtensionManifest,
	ExtensionUpdate,
	ExtensionPermission,
	ExtensionPermissionKind,
	ExtensionSummary,
	HookKind,
	ImageUpload,
	IntelligenceAgent,
	IntelligenceAgentCue,
	IntelligenceAgentTeam,
	Attachment,
	IntelligenceAgentMessage,
	IntelligenceAgentRun,
	IntelligenceCostEvent,
	IntelligenceDefault,
	IntelligenceSpend,
	SpendPeriod,
	IntelligenceThread,
	IntelligenceThreadMessage,
	ThreadItem,
	Memory,
	Article,
	ArticleFilter,
	ArticleSummary,
	Feed,
	FeedCorpusHit,
	FeedSummary,
	ArticleFetchFailure,
	IntelligenceEndpointPreset,
	LongCache,
	OrganizationUnit,
	RawRequest,
	RedactedConnectorAccount,
	RedactedIntelligenceEndpoint,
	RegistryResult,
	Replies,
	ReplyFrame,
	Requests,
	RequestType,
	RefetchedArticle,
	SavedTaskView,
	SearchResult,
	Send,
	Tag,
	TagFilter,
	Task,
	TaskFolder,
	Version,
	VersionSummary,
	DeletedItem,
	TaskStatus,
	VersionAuthor,
	VersionChange,
	TaskPriority,
	TaskSort,
	Vocabulary,
	Workspace,
	WorkspaceResourceKind
} from './types';
import vocabulary from './vocabulary.json';

/** The daemon's words, as its `hello` would carry them. */
export const MOCK_VOCABULARY = vocabulary as Vocabulary;

const DAY = 86_400;

function localMidnight(daysAgo: number, now = new Date()): number {
	const date = new Date(now.getFullYear(), now.getMonth(), now.getDate() - daysAgo);
	return Math.floor(date.getTime() / 1000);
}

function ymd(seconds: number): string {
	const d = new Date(seconds * 1000);
	const pad = (n: number) => String(n).padStart(2, '0');
	return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

function nowSeconds(): number {
	return Math.floor(Date.now() / 1000);
}

/** The local day `days` days from today, as "2026-10-12". */
function dayIn(days: number): string {
	const d = new Date();
	return ymd(new Date(d.getFullYear(), d.getMonth(), d.getDate() + days).getTime() / 1000);
}

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

/** A stored message the way the daemon writes one. */
function message(
	id: number,
	thread_id: number,
	created_at: number,
	item: TranscriptItem
): IntelligenceThreadMessage {
	return { id, thread_id, created_at, item: storedItem(item) };
}

/** A transcript item as a thread message holds it. */
function storedItem(item: TranscriptItem): ThreadItem {
	if ('UserMessage' in item) return { kind: 'userMessage', text: item.UserMessage.text };
	if ('AssistantMessage' in item) {
		return { kind: 'assistantMessage', text: item.AssistantMessage.text };
	}
	if ('Thinking' in item) {
		const { thought, signature, in_content } = item.Thinking.Standard;
		return { kind: 'thought', text: thought, signature, in_content };
	}
	if ('ToolCallRequest' in item) return { kind: 'toolCallRequest', ...item.ToolCallRequest };
	const { call_identifier, response } = item.ToolCallResponse;
	return 'Success' in response
		? { kind: 'toolCallResponse', call_identifier, succeeded: true, text: response.Success }
		: { kind: 'toolCallResponse', call_identifier, succeeded: false, text: response.Failure };
}

/** One model call to Claude Sonnet 4.5 ($3 in, $15 out per 1M), half its prompt cached. */
function securityEvent(
	id: number,
	created_at: number,
	kind: SecurityEventKind,
	client_id: number | null,
	client_name: string | null,
	address: string | null
): SecurityEvent {
	return { id, created_at, kind, client_id, client_name, address };
}

/** How far back each spend period counts, in seconds, shortest first. */
const SPEND_PERIODS: [SpendPeriod, number][] = [
	['hour', 3600],
	['day', DAY],
	['month', 30 * DAY]
];

/** What the cost events of each period add up to, against its limit, as the daemon sums them. */
function spend(state: MockState): IntelligenceSpend[] {
	const now = nowSeconds();
	return SPEND_PERIODS.map(([period, seconds]) => {
		const spent = state.costEvents
			.filter((e) => e.created_at > now - seconds)
			.reduce((sum, e) => sum + e.total_cost, 0);
		const limit = state.spendLimits.find((l) => l.period === period)?.amount ?? null;
		return { period, limit, spent, reached: limit !== null && spent >= limit };
	});
}

function costEvent(
	id: number,
	thread_id: number,
	created_at: number,
	prompt: number,
	output: number
): IntelligenceCostEvent {
	const cached = Math.floor(prompt / 2);
	const input_cost = ((prompt - cached) * 3) / 1e6;
	const output_cost = (output * 15) / 1e6;
	const cache_read_cost = (cached * 0.3) / 1e6;
	return {
		id,
		created_at,
		thread_id,
		provider_identifier: 'anthropic',
		model_identifier: 'claude-sonnet-4-5',
		input_tokens: prompt - cached,
		output_tokens: output,
		cache_read_tokens: cached,
		cache_write_tokens: 0,
		total_cost: input_cost + output_cost + cache_read_cost,
		input_cost,
		output_cost,
		cache_read_cost,
		cache_write_cost: 0
	};
}

/** What each built-in preset resolves to, as the daemon's `ProviderDefaults` say. */
const PRESET_DEFAULTS: Record<
	Exclude<IntelligenceEndpointPreset['preset'], 'custom'>,
	{
		name: string;
		provider_identifier: string;
		dialect: Dialect;
		default_model: string;
		base_url: string;
		usage_url: string;
		cache_minutes: number;
		long_cache: LongCache | null;
	}
> = {
	openAi: {
		name: 'OpenAI',
		provider_identifier: 'openai',
		dialect: 'responses',
		default_model: 'gpt-6.1-sol',
		base_url: 'https://api.openai.com/v1',
		usage_url: 'https://platform.openai.com/usage',
		cache_minutes: 30,
		long_cache: { minutes: 720, write_multiple: 2 }
	},
	anthropic: {
		name: 'Anthropic',
		provider_identifier: 'anthropic',
		dialect: 'messages',
		default_model: 'claude-sonnet-5-5',
		base_url: 'https://api.anthropic.com/v1',
		usage_url: 'https://platform.claude.com/usage',
		cache_minutes: 5,
		long_cache: { minutes: 60, write_multiple: 1.6 }
	},
	xai: {
		name: 'xAI',
		provider_identifier: 'xai',
		dialect: 'responses',
		default_model: 'grok-4.7',
		base_url: 'https://api.x.ai/v1',
		usage_url: 'https://console.x.ai/team/default/usage',
		cache_minutes: 5,
		long_cache: null
	},
	mistral: {
		name: 'Mistral',
		provider_identifier: 'mistral',
		dialect: 'chatCompletions',
		default_model: 'mistral-large-latest',
		base_url: 'https://api.mistral.ai/v1',
		usage_url: 'https://admin.mistral.ai/organization/usage',
		cache_minutes: 5,
		long_cache: null
	},
	fireworks: {
		name: 'Fireworks',
		provider_identifier: 'fireworks-ai',
		dialect: 'chatCompletions',
		default_model: 'accounts/fireworks/models/deepseek-v4p1-flash',
		base_url: 'https://api.fireworks.ai/inference/v1',
		usage_url: 'https://app.fireworks.ai/account/usage',
		cache_minutes: 5,
		long_cache: null
	},
	deepInfra: {
		name: 'DeepInfra',
		provider_identifier: 'deepinfra',
		dialect: 'chatCompletions',
		default_model: 'deepseek-ai/DeepSeek-V4.1-Flash',
		base_url: 'https://api.deepinfra.com/v1/openai',
		usage_url: 'https://deepinfra.com/dash/usage',
		cache_minutes: 5,
		long_cache: null
	},
	togetherAI: {
		name: 'Together AI',
		provider_identifier: 'togetherai',
		dialect: 'chatCompletions',
		default_model: 'deepseek-ai/DeepSeek-V4.1-Flash',
		base_url: 'https://api.together.ai/v1',
		usage_url: 'https://api.together.ai/settings/organization/~current/billing',
		cache_minutes: 5,
		long_cache: null
	},
	openRouter: {
		name: 'OpenRouter',
		provider_identifier: 'openrouter',
		dialect: 'chatCompletions',
		default_model: 'anthropic/claude-sonnet-5.5',
		base_url: 'https://openrouter.ai/api/v1',
		usage_url: 'https://openrouter.ai/activity',
		cache_minutes: 5,
		long_cache: null
	}
};

/** An endpoint model linked to catalog row `catalog`, with its values, as the daemon's read joins them. */
function fromCatalog(id: number, endpoint_id: number, catalog: CatalogModel): EndpointModel {
	return {
		id,
		endpoint_id,
		model_identifier: catalog.model_identifier,
		model_name: catalog.model_name,
		catalog_model_id: catalog.id,
		context_size: catalog.context_size,
		input_token_cost: catalog.input_token_cost,
		output_token_cost: catalog.output_token_cost,
		cache_read_token_cost: catalog.cache_read_token_cost,
		cache_write_token_cost: catalog.cache_write_token_cost,
		large_context_threshold: catalog.large_context_threshold,
		large_context_input_token_cost: catalog.large_context_input_token_cost,
		large_context_output_token_cost: catalog.large_context_output_token_cost,
		large_context_cache_read_token_cost: catalog.large_context_cache_read_token_cost,
		large_context_cache_write_token_cost: catalog.large_context_cache_write_token_cost
	};
}

/** A script a registry points at: its text and the manifest the daemon reads from it. */
interface MockSource {
	script: string;
	manifest: ExtensionManifest;
}

/**
 * What an installed registry extension's `registryResults` returns, or the
 * error it throws. Its `registryLookup` answers from the same results.
 */
interface MockRegistry {
	extension_id: number;
	results: RegistryResult[];
	error: string | null;
}

/** A plausible extension script whose `manifest()` returns `manifest`. */
function extensionScript(manifest: ExtensionManifest): string {
	return `function manifest() {
  return ${JSON.stringify(manifest, null, 2).replaceAll('\n', '\n  ')};
}

function onLaunch() {
  log.info("${manifest.name} started");
}
`;
}

function extensionManifest(
	identifier: string,
	name: string,
	version: number,
	author: string,
	description: string,
	permissions: ExtensionPermissionKind[],
	hooks: HookKind[] = []
): ExtensionManifest {
	return {
		identifier,
		name,
		version,
		description,
		author,
		icon: null,
		permissions,
		hooks,
		service: null
	};
}

/**
 * The scripts two registries list. "Todoist" lies: its registry entry
 * claims an older version, read-only access and no hooks, but the script
 * also asks for `cohesion:write` and `task:create`.
 */
function seedSources(): Map<string, MockSource> {
	const manifests: [string, ExtensionManifest][] = [
		[
			'https://registry.example.com/ext/linear.js',
			extensionManifest(
				'com.cohesion.linear',
				'Linear',
				3,
				'Cohesion',
				'Import Linear issues.',
				['cohesion:write', 'network'],
				['journal:settled']
			)
		],
		[
			'https://registry.example.com/ext/todoist.js',
			extensionManifest(
				'com.cohesion.todoist',
				'Todoist',
				5,
				'Cohesion',
				'Two-way Todoist sync.',
				['cohesion:read', 'cohesion:write'],
				['task:create']
			)
		],
		[
			'https://registry.example.com/ext/ios-companion.js',
			extensionManifest(
				'com.cohesion.cloud.ios-companion',
				'Cohesion Link for iOS',
				9,
				'Cohesion',
				'Forwards your agents’ messages and reactions, extension updates, and the notifications they send to your iPhone.',
				['cohesion:read', 'timer'],
				['launch', 'task:create']
			)
		]
	];
	return new Map(
		manifests.map(([url, manifest]) => [url, { script: extensionScript(manifest), manifest }])
	);
}

function seedRegistries(sources: Map<string, MockSource>): MockRegistry[] {
	const listed = (url: string, claim: Partial<ExtensionManifest> = {}): RegistryResult => ({
		manifest: { ...sources.get(url)!.manifest, ...claim },
		source_url: url
	});
	return [
		{
			extension_id: 2,
			results: [
				listed('https://registry.example.com/ext/linear.js'),
				listed('https://registry.example.com/ext/todoist.js', {
					version: 4,
					permissions: ['cohesion:read'],
					hooks: []
				}),
				listed('https://registry.example.com/ext/ios-companion.js')
			],
			error: null
		},
		{
			extension_id: 3,
			results: [],
			error:
				'network.fetch failed: 503 Service Unavailable (https://plugins.internal.example/index.json)'
		}
	];
}

/** A record a connected service holds, as the connector's `getResource` returns it. */
interface MockRemoteResource {
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
interface MockConnector {
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

const NOTION_ICON =
	'data:image/svg+xml,' +
	encodeURIComponent(
		'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><rect width="100" height="100" rx="20" fill="#191919"/><text x="50" y="68" font-size="56" font-weight="700" text-anchor="middle" fill="#fff" font-family="Georgia, serif">N</text></svg>'
	);

/**
 * The Launch plan page as its connector draws it: every kind of block, at
 * each heading level, with a checkbox and a link inside overviews.
 */
function launchPlanView(): Component {
	return {
		blocks: [
			{ kind: 'heading', level: 1, text: 'Launch plan' },
			{
				kind: 'text',
				text: 'Everything the launch needs, in one place.\nUpdated by the team every Friday.'
			},
			{ kind: 'overview', title: 'Status', child: { kind: 'text', text: 'In progress' } },
			{ kind: 'overview', title: 'Owner', child: { kind: 'text', text: 'Ada Lovelace' } },
			{ kind: 'overview', title: 'Approved', child: { kind: 'checkbox', checked: true } },
			{
				kind: 'overview',
				title: 'Tracker',
				child: { kind: 'link', text: 'Open in Notion', destination: 'https://www.notion.so' }
			},
			{ kind: 'heading', level: 2, text: 'Checklist' },
			{ kind: 'checkbox', checked: true, text: 'Write the announcement' },
			{ kind: 'checkbox', checked: false, text: 'Record the demo' },
			{ kind: 'checkbox', checked: false, text: 'Tell the beta testers' },
			{ kind: 'heading', level: 3, text: 'Links' },
			{ kind: 'link', text: 'Press kit', destination: 'https://example.com/press' },
			{ kind: 'link', text: 'Email the team', destination: 'mailto:team@example.com' }
		]
	};
}

/**
 * Two connectors: Notion (its extension has an icon), with two units and two
 * records (a page and a JSON database), and Team wiki, whose extension has no
 * icon, so its tile shows.
 */
function seedConnectors(now: number): MockConnector[] {
	return [
		{
			extension_id: 4,
			title: 'Notion',
			unit: 'Teamspace',
			error: null,
			account: { identifier: 'notion-acme', description: 'Acme Inc.' },
			units: [
				{ id: 'eng', display: 'Engineering' },
				{ id: 'ops', display: 'Operations' }
			],
			kinds: [
				['page', 'Page', 'Pages'],
				['database', 'Database', 'Databases']
			],
			resources: [
				{
					identifier: 'page-roadmap',
					unit: 'eng',
					title: 'Q4 roadmap',
					kind_identifier: 'page',
					content_kind: 'markdown',
					contents:
						'# Q4 roadmap\n\n- Ship connectors\n- Tidy the inbox\n\nOwners meet **every Monday**.',
					last_updated: now - 2 * DAY
				},
				{
					identifier: 'page-launch',
					unit: 'eng',
					title: 'Launch plan',
					kind_identifier: 'page',
					content_kind: 'markdown',
					contents: '# Launch plan\n\nStatus: in progress. Owner: Ada Lovelace.',
					last_updated: now - 3 * 3600,
					render: launchPlanView()
				},
				{
					identifier: 'page-playlist',
					unit: 'eng',
					title: 'Road trip playlist',
					kind_identifier: 'page',
					content_kind: 'markdown',
					contents: 'Tracks to drive to.',
					last_updated: 0
				},
				{
					identifier: 'db-budget',
					unit: 'ops',
					title: 'Budget',
					kind_identifier: 'database',
					content_kind: 'json',
					contents: '{"rows":[{"item":"Hosting","amount":120},{"item":"Domains","amount":30}]}',
					last_updated: now - 5 * DAY
				}
			]
		},
		{
			extension_id: 5,
			title: 'Team wiki',
			unit: 'Space',
			error: null,
			account: { identifier: 'wiki-main', description: 'wiki.example.com' },
			units: [{ id: 'all', display: 'Everything' }],
			kinds: [['article', 'Article', 'Articles']],
			resources: [
				{
					identifier: 'onboarding',
					unit: 'all',
					title: 'Onboarding',
					kind_identifier: 'article',
					content_kind: 'markdown',
					contents: 'Welcome aboard. Start with the **handbook**.',
					last_updated: now - DAY
				},
				// A connector with no modified date reports 0 (unknown).
				{
					identifier: 'playlist',
					unit: 'all',
					title: 'Road trip playlist',
					kind_identifier: 'article',
					content_kind: 'markdown',
					contents: 'Tracks to drive to.',
					last_updated: 0
				}
			]
		}
	];
}

/**
 * An account as the daemon stores it; its connector and whether it can
 * import are added when it is sent.
 */
type MockConnectorAccount = Omit<RedactedConnectorAccount, 'connector' | 'importable'> & {
	connector_id: number;
};

/** A page as the mock fetched it, like the daemon's `ArticleCapture`. */
interface MockCapture {
	final_url: string;
	page_title: string | null;
	text: string;
	truncated: boolean;
}

interface MockState {
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
	agentRuns: IntelligenceAgentRun[];
	activities: Activity[];
	actors: Actor[];
	actorSessions: ActorSession[];
	actorSessionMessages: ActorSessionMessage[];
	actorSessionAttachments: Attachment[];
	memories: Memory[];
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
}

type MockContactMethod = Omit<ContactMethod, 'workspace_id'>;

/** Whether `day` matches `filter`, as the daemon's day filters do: a task with no day never matches. */
function onDay(day: string | null, filter: DayFilter): boolean {
	if (day === null) return false;
	if (typeof filter === 'string') return day === filter;
	if (Array.isArray(filter)) return filter.includes(day);
	return (
		(filter.before == null || day < filter.before) && (filter.after == null || day > filter.after)
	);
}

function seed(): MockState {
	const now = nowSeconds();
	const sources = seedSources();
	const journal = (
		id: number,
		workspace_id: number,
		daysAgo: number,
		contents: string
	): Document => {
		const day = localMidnight(daysAgo);
		return {
			id,
			workspace_id,
			document_type: 'journal',
			title: ymd(day),
			contents,
			modified: day + 18 * 3600,
			journal_date: ymd(day),
			deleted: false,
			folder_id: null
		};
	};
	// Filed in `documentFolders` below: Research holds Search and 8 and 9, Search holds 5, and 6 is in the Inbox.
	const filed: Record<number, number> = { 5: 3, 8: 2, 9: 2 };
	const doc = (id: number, workspace_id: number, title: string, contents: string): Document => ({
		id,
		workspace_id,
		document_type: 'normal',
		title,
		contents,
		modified: now - id * 3600,
		journal_date: null,
		deleted: false,
		folder_id: filed[id] ?? null
	});
	const task = (
		id: number,
		title: string,
		status: Task['status'],
		task_folder_id: number | null,
		workspace_id = 1,
		priority: Task['priority'] = 'none',
		description = '',
		size: Task['size'] = null,
		assignee: Task['assignee'] = null
	): Task => ({
		id,
		workspace_id,
		task_folder_id,
		title,
		description,
		status,
		priority,
		size,
		assignee,
		start_date: null,
		due_date: null,
		deleted: false,
		tag_ids: []
	});
	const catalogModel = (
		id: number,
		provider_identifier: string,
		provider_name: string,
		model_identifier: string,
		model_name: string,
		input: number,
		output: number
	): CatalogModel => ({
		id,
		provider_identifier,
		provider_name,
		model_identifier,
		model_name,
		context_size: 200_000,
		input_token_cost: input,
		output_token_cost: output,
		cache_read_token_cost: input / 10,
		cache_write_token_cost: input * 1.25,
		large_context_threshold: null,
		large_context_input_token_cost: null,
		large_context_output_token_cost: null,
		large_context_cache_read_token_cost: null,
		large_context_cache_write_token_cost: null
	});

	const catalog = [
		catalogModel(1, 'anthropic', 'Anthropic', 'claude-sonnet-5-5', 'Claude Sonnet 5.5', 2, 10),
		catalogModel(2, 'anthropic', 'Anthropic', 'claude-opus-5-5', 'Claude Opus 5.5', 4, 20),
		catalogModel(3, 'anthropic', 'Anthropic', 'claude-haiku-5-5', 'Claude Haiku 5.5', 0.1, 0.5),
		catalogModel(4, 'openai', 'OpenAI', 'gpt-6.1-terra', 'GPT-6.1 Terra', 1, 5),
		catalogModel(5, 'openai', 'OpenAI', 'gpt-6-luna', 'GPT-6 Luna', 0.1, 0.5),
		catalogModel(6, 'xai', 'xAI', 'grok-4.7', 'Grok 4.7', 2, 6)
	];

	return {
		// Modelled on a real daemon's data (the Cohesion tracker, a church
		// volunteer rota and an empty work workspace), with people made up.
		workspaces: [
			{ id: 1, title: 'Cohesion', emoji: '📱', deleted: false },
			{ id: 2, title: 'Church', emoji: '⛪', deleted: false },
			{ id: 3, title: 'Containers', emoji: '📦', deleted: false }
		],
		documents: [
			journal(
				1,
				1,
				0,
				'## 10:58 — CI for the consolidated repos\n\n[CI session](cohesion://actor-session/2) green in five repos: cohesion `e5a7406`, cloud `d4d8f37`, relay `7d6dd4c`, extensions `5688d84` and actor-codex `9366deb`.\n\n## 11:05 — Libraries public on MIT\n\n`cohesion-ts` and `cohesion-agent-kit` are public under MIT, renamed to `cohesionhq/ts` and `cohesionhq/actor-kit`.\n'
			),
			journal(
				2,
				1,
				1,
				'## Install extension from URL\n\n- **Impact:** the Extensions page gains **Install from URL…** beside Install from file…. The daemon downloads the script under a 5 MB, 20 second cap, and the same review opens.\n- **Status:** checks pass; pushed as `2c06a1b`.\n'
			),
			journal(
				3,
				1,
				2,
				'## Allow agents to message each other (`e99e1ae`)\n\nAgents ask each other with an `ask_agent` tool. The asked agent answers in a fork of its conversation, and its final reply comes back as the answer within 10 minutes.\n'
			),
			journal(
				4,
				1,
				9,
				'## Rewind\n\nTask and document history is in. Retention is the "Rewind versions" row under Settings › Storage, seven days by default.\n'
			),
			doc(
				5,
				1,
				'Full-text search syntax and errors',
				'# Full-text search syntax and errors\n\nWorkspace, feed-corpus and actor-session search use SQLite FTS5 query syntax. Punctuation such as the hyphen in `re-auth` can be read as syntax; search for the phrase with `w.search(\'"re-auth"\')` when a literal is meant.\n\nA malformed query returns a validation error with a quoting suggestion instead of a generic internal error.\n'
			),
			doc(
				6,
				1,
				'Extensions Registry',
				'# Extensions Registry\n\nExtensions are only installed by hand today. A registry works like apt, dnf or pacman: an extension with the `registry` permission answers `registryResults(query)` with the extensions that match.\n\n```mermaid\nflowchart LR\n  Search --> Registry[registryResults] --> Review[Review the real manifest] --> Install\n```\n\nThe listed manifest is only a claim. Installing downloads the script, reads its real manifest, and shows both for the user to accept or deny, highlighting any difference.\n'
			),
			doc(
				7,
				2,
				'Production team rotation',
				'Each Sunday needs a producer, a run-through lead, sound, lighting, Pro-Presenter and camera. Planning Center sends the rota; tasks follow it.\n'
			),
			doc(
				8,
				1,
				'JS script engine pool sizes',
				'# JS script engine pool sizes\n\nSource: `cohesion-js/src/engine/script_engine.rs`, `ScriptEngine::new`.\n\n`workers = max(available_parallelism, 4)`, so a machine with 4 or fewer cores gets 4.\n\n| Pool | Used for | Size |\n|---|---|---|\n| General | general-purpose scripts | `workers` |\n| Foreground agent | code tool scripts in an agent conversation | `workers` |\n| Background agent | background runs and cue functions | `workers * 2` |\n| Isolated | isolated scripts | 1 |\n\nNot configurable: no env var or setting overrides the count.\n'
			),
			doc(
				9,
				1,
				'Rewind: storage and lifecycle',
				'# Rewind: storage and lifecycle\n\n- Rewind stores full snapshots, not diffs: `task_versions` and `document_versions` (including journals).\n- A plain edit within 120 seconds of the newest version overwrites that version. Deletion and restore each create a separate version.\n- Retention is the "Rewind versions" row under Retention in Settings › Storage, seven days by default. Cleanup runs at startup and hourly.\n- Restore copies the tracked fields back and undeletes the record. A task whose folder is gone returns to Inbox.\n'
			),
			doc(
				10,
				1,
				'Launch plan',
				'# Launch plan\n\nWhat has to land before the beta, as cards. A link in a sentence, such as [the search notes](cohesion://document/1/5), stays a link.\n\n[Install extension from URL](cohesion://task/1/4)\n[Cohesion-bench](cohesion://task/1/2)\n\n## Who is on it\n\n[Secretary](cohesion://agent/2)\n[the install session](cohesion://actor-session/1)\n\n## Reading\n\n[Notion page](cohesion://resource/1/1)\n[Article](cohesion://article/1)\n\n## Dropped\n\n[Old migration task](cohesion://task/1/999)\n'
			)
		],
		documentFolders: [
			{ id: 1, workspace_id: 1, parent_id: null, title: 'Design', sort_order: 1, deleted: false },
			{ id: 2, workspace_id: 1, parent_id: null, title: 'Research', sort_order: 2, deleted: false },
			{ id: 3, workspace_id: 1, parent_id: 2, title: 'Search', sort_order: 1, deleted: false }
		],
		folders: [
			{ id: 1, workspace_id: 1, title: 'Core', sort_order: 1, deleted: false },
			{ id: 2, workspace_id: 1, title: 'WebUI', sort_order: 2, deleted: false },
			{ id: 3, workspace_id: 1, title: 'Plus', sort_order: 3, deleted: false },
			{ id: 4, workspace_id: 1, title: 'Cloud', sort_order: 4, deleted: false },
			{ id: 5, workspace_id: 1, title: 'JavaScript', sort_order: 5, deleted: false },
			{ id: 6, workspace_id: 1, title: 'WebUI Harnesses', sort_order: 6, deleted: false },
			{ id: 7, workspace_id: 2, title: 'PCO', sort_order: 1, deleted: false }
		],
		tasks: [
			{
				...task(
					1,
					'Configure and deploy the Linear connector',
					'incomplete',
					null,
					1,
					'none',
					'The `com.cohesion.linear` extension is built and pushed but not live.\n\n- register the OAuth app\n- install it on the daemon',
					null,
					{ kind: 'user' }
				),
				due_date: dayIn(0)
			},
			{
				...task(2, 'Cohesion-bench', 'incomplete', null, 1, 'high'),
				due_date: dayIn(2)
			},
			{
				...task(
					3,
					'Add a command to manually rebuild an agent’s context',
					'incomplete',
					1,
					1,
					'medium',
					'',
					2,
					{
						kind: 'agent',
						agent_id: 1
					}
				),
				start_date: dayIn(0)
			},
			task(4, 'Install extension from URL', 'completed', 1),
			task(5, 'Change agent status to just be working if it’s in progress', 'cancelled', 1),
			{
				...task(6, 'Persist research text when going back', 'incomplete', 2, 1, 'none', '', null, {
					kind: 'contact',
					contact_id: 1
				}),
				due_date: dayIn(-1)
			},
			task(7, 'Unify the shimmer and blink on one faster clock', 'completed', 2),
			task(8, 'Confirm the December 6 run-through crew', 'incomplete', null, 2),
			task(
				9,
				'Repeating tasks',
				'incomplete',
				1,
				1,
				'high',
				'Needs design: recurrence rules, what completing a repeat does, and how extensions like Todoist map. See [Area: Tasks](cohesion://document/1/5).',
				4
			),
			task(10, 'Subagent sessions', 'incomplete', 1, 1, 'medium'),
			task(
				11,
				'hellocohesion.com landing page',
				'incomplete',
				1,
				1,
				'low',
				'In the style of Linear: a new repo, deployed on Vercel.',
				3
			),
			task(
				12,
				'Themes/flair',
				'incomplete',
				2,
				1,
				'none',
				'Let the user customize the look of the web UI.\n\n- Font\n  - System\n  - Monospace\n  - Serif'
			),
			task(
				13,
				'Add memories dialog into agents, in their overflow menu',
				'incomplete',
				2,
				1,
				'low',
				'This should answer “What memories can this agent see?”, and delete from there too.',
				2
			),
			task(
				14,
				'Voice capture',
				'incomplete',
				3,
				1,
				'high',
				'Transcribe with a better model than the on-device one, such as a local Whisper.',
				3
			),
			task(
				15,
				'Modernize and standardize the composers in Cohesion+',
				'incomplete',
				3,
				1,
				'medium',
				'',
				3
			),
			task(16, 'Plus: remove article saving; feeds stay a reader', 'completed', 3),
			task(
				17,
				'Cloud Bridge',
				'incomplete',
				4,
				1,
				'low',
				'A persistent two-way connection between Cloud and cohesiond, without special-casing Cohesion Cloud.'
			),
			task(
				18,
				'Idea: slack or discord bots, one created per agent',
				'incomplete',
				4,
				1,
				'high',
				'',
				4
			),
			task(
				19,
				'FormKit',
				'incomplete',
				5,
				1,
				'high',
				'A protocol the JS runtime uses to create user interfaces: a title, an optional description, controls and a submit. Text fields and pickers first.'
			),
			task(
				20,
				'Linux QT web harness',
				'incomplete',
				6,
				1,
				'medium',
				'A platform-appropriate copy of the Apple WebUI harness for Linux that looks good on KDE and GNOME.',
				3
			),
			task(21, 'Dashboard: recap and quick dispatch like the reworked WebUI', 'completed', 3),
			...(
				[
					[22, 'December 6: Sam Rivera (Lighting)', 29],
					[23, 'December 6: Priya Shah (Pro-Presenter)', 29],
					[24, 'December 6: Producer', 29],
					[25, 'December 13: Marcus Lee (Sound Engineer)', 36],
					[26, 'December 13: Producer', 36],
					[27, 'December 20: Dana Cho (Camera)', 43]
				] as const
			).map(([id, title, days]) => ({
				...task(id, title, 'incomplete', 7, 2),
				due_date: dayIn(days)
			}))
		],
		endpoints: [
			{
				id: 1,
				name: 'Anthropic',
				backend: {
					kind: 'integrated',
					provider_identifier: 'anthropic',
					dialect: 'messages',
					base_url: 'https://api.anthropic.com/v1',
					usage_url: null,
					cache_minutes: 5,
					long_cache: { minutes: 60, write_multiple: 1.6 }
				}
			},
			{
				id: 2,
				name: 'OpenAI',
				backend: {
					kind: 'integrated',
					provider_identifier: 'openai',
					dialect: 'responses',
					base_url: 'https://api.openai.com/v1',
					usage_url: null,
					cache_minutes: 30,
					long_cache: { minutes: 720, write_multiple: 2 }
				}
			},
			{
				id: 3,
				name: 'XAI',
				backend: {
					kind: 'integrated',
					provider_identifier: 'xai',
					dialect: 'chatCompletions',
					base_url: 'https://api.x.ai/v1',
					usage_url: null,
					cache_minutes: 5,
					long_cache: null
				}
			}
		],
		endpointModels: [
			fromCatalog(1, 1, catalog[0]),
			fromCatalog(2, 1, catalog[2]),
			fromCatalog(3, 2, catalog[4]),
			fromCatalog(4, 2, catalog[3]),
			fromCatalog(5, 3, catalog[5])
		],
		defaults: [
			{ id: 1, tier: 'agent', endpoint_model_id: 1 },
			{ id: 2, tier: 'low', endpoint_model_id: 3 },
			{ id: 3, tier: 'medium', endpoint_model_id: 2 },
			{ id: 4, tier: 'high', endpoint_model_id: 1 }
		],
		catalog,
		threads: [
			{
				id: 1,
				created_at: now - 2 * DAY,
				prompt_id: 1,
				system_prompt: '',
				origin: { kind: 'root' },
				remote: null
			},
			{
				id: 2,
				created_at: now - 3600,
				prompt_id: 1,
				system_prompt: '',
				origin: { kind: 'root' },
				remote: null
			}
		],
		messages: [
			message(1, 1, now - 2 * DAY, { UserMessage: { text: 'What’s open in Core?' } }),
			message(2, 1, now - 2 * DAY + 2, {
				Thinking: {
					Standard: {
						thought: 'Look up the open tasks in the Core folder.',
						signature: null,
						in_content: false
					}
				}
			}),
			message(3, 1, now - 2 * DAY + 3, {
				ToolCallRequest: {
					call_identifier: 'call_seed_1',
					tool_name: 'code-tool',
					arguments: JSON.stringify({
						code: "cohesion.tasks({ folder: 1, statuses: ['incomplete'] }).map((t) => t.title)"
					})
				}
			}),
			message(4, 1, now - 2 * DAY + 4, {
				ToolCallResponse: {
					call_identifier: 'call_seed_1',
					response: {
						Success: '["Add a command to manually rebuild an agent’s context","Repeating tasks"]'
					}
				}
			}),
			message(5, 1, now - 2 * DAY + 5, {
				AssistantMessage: {
					text: 'You have **Add a command to manually rebuild an agent’s context** and **Repeating tasks**.'
				}
			}),
			message(6, 2, now - 3600, { UserMessage: { text: 'What shipped yesterday?' } }),
			message(7, 2, now - 3595, {
				AssistantMessage: { text: 'Install from URL, pushed as `2c06a1b`.' }
			})
		],
		costEvents: [
			costEvent(1, 1, now - 2 * DAY + 3, 6_200, 140),
			costEvent(2, 1, now - 2 * DAY + 5, 6_600, 320),
			costEvent(3, 2, now - 3595, 5_900, 210)
		],
		spendLimits: [{ period: 'month', amount: 20 }],
		clients: [
			{
				id: 1,
				deleted: false,
				key_kind: 'certificate',
				key: 'MCowBQYDK2VwAyEA…',
				client_name: 'iPhone',
				created_at: now - 40 * DAY,
				last_seen_at: now - 2 * 3600,
				last_address: '100.101.3.7'
			},
			{
				id: 2,
				deleted: false,
				key_kind: 'certificate',
				key: '3f9a1c07be52d46e',
				client_name: 'MBP',
				created_at: now - 90 * DAY,
				last_seen_at: now - 300,
				last_address: '192.168.1.20'
			},
			// On the daemon's own machine, identified by a local key on its socket;
			// paired before pairing was dated.
			{
				id: 3,
				deleted: false,
				key_kind: 'local',
				key: '9c41e07a2b6d58f3',
				client_name: 'apollo',
				created_at: null,
				last_seen_at: now - 60,
				last_address: null
			}
		],
		// The MBP, offering its models and notes.
		onlineClients: [2],
		// The page is served by the CLI on apollo, as `cohesion open` does.
		currentClient: 3,
		security: {
			id: 1,
			pairing_opened_by: 'anyClient',
			alert_new_client: true,
			revoke_inactive_after_days: null
		},
		storage: {
			id: 1,
			thread_retention_days: 14,
			actor_session_retention_days: 7,
			rewind_retention_days: 7,
			deleted_item_retention_days: 30,
			memory_limit: 20
		},
		versions: [],
		databaseSize: {
			bytes: 48_234_496,
			free_bytes: 1_310_720,
			areas: [
				{ area: 'agentChats', bytes: 21_430_272 },
				{ area: 'actorSessions', bytes: 12_582_912 },
				{ area: 'feedsAndArticles', bytes: 6_291_456 },
				{ area: 'rewindVersions', bytes: 2_621_440 },
				{ area: 'documentsAndTasks', bytes: 1_835_008 },
				{ area: 'resources', bytes: 393_216 },
				{ area: 'memories', bytes: 49_152 },
				{ area: 'other', bytes: 1_720_320 }
			]
		},
		profile: { id: 1, name: null, bio: null, photo: { kind: 'none' } },
		securityEvents: [
			securityEvent(1, now - 40 * DAY, 'clientPaired', 1, 'iPhone', '192.168.1.31'),
			securityEvent(2, now - 3 * DAY, 'settingsChanged', null, null, null),
			securityEvent(3, now - 26 * 3600, 'pairingRefused', null, null, '203.0.113.54'),
			securityEvent(4, now - 2 * 3600, 'pairingOpened', 2, 'MBP', '192.168.1.20')
		],
		extensions: [
			{
				id: 1,
				name: 'Cohesion Link for iOS',
				version: 8,
				script_identifier: 'com.cohesion.cloud.ios-companion',
				priority: 0,
				description:
					'Forwards your agents’ messages and reactions, extension updates, and the notifications they send to your iPhone.',
				author: 'Cohesion',
				icon: null
			},
			{
				id: 2,
				name: 'Cohesion Cloud Registry',
				version: 1,
				script_identifier: 'com.cohesion.cloud.registry',
				priority: 0,
				description: 'Find and update extensions published on Cohesion Cloud.',
				author: 'Cohesion',
				icon: null
			},
			{
				id: 3,
				name: 'Internal plugins',
				version: 1,
				script_identifier: 'com.acme.internal-plugins',
				priority: 1,
				description: 'Lists the team’s private extensions.',
				author: '',
				icon: null
			},
			{
				id: 4,
				name: 'Notion',
				version: 1,
				script_identifier: 'com.example.notion',
				priority: 0,
				description: 'Imports pages and databases from Notion.',
				author: 'Example Co',
				icon: NOTION_ICON
			},
			{
				id: 5,
				name: 'Team wiki',
				version: 2,
				script_identifier: 'com.example.wiki',
				priority: 0,
				description: 'Imports articles from the team wiki.',
				author: '',
				icon: null
			}
		],
		permissions: [
			...MOCK_VOCABULARY.permissions
				.map((p) => p.kind)
				.filter((p) => !['registry', 'oauth', 'connector'].includes(p))
				.map((permission) => ({
					extension_id: 1,
					permission,
					enabled: permission === 'cohesion:read' || permission === 'timer'
				})),
			...[2, 3].flatMap((extension_id) =>
				(['network', 'registry'] as const).map((permission) => ({
					extension_id,
					permission,
					enabled: true
				}))
			),
			...[4, 5].flatMap((extension_id) =>
				(['network', 'oauth', 'connector'] as const).map((permission) => ({
					extension_id,
					permission,
					enabled: true
				}))
			)
		].map((row, i) => ({ id: i + 1, ...row })),
		hooks: [
			...MOCK_VOCABULARY.hooks.map(({ kind: hook }) => ({
				extension_id: 1,
				hook,
				enabled: hook === 'launch' || hook === 'task:create'
			})),
			...(['resource:import', 'resource:update'] as const).map((hook) => ({
				extension_id: 4,
				hook,
				enabled: hook === 'resource:import'
			}))
		].map((row, i) => ({ id: i + 1, ...row })),
		sources,
		registries: seedRegistries(sources),
		...seedAgents(now),
		...seedActors(now),
		actorSessionAttachments: [],
		...seedConnectorData(now),
		articles: seedArticles(now),
		feeds: seedFeeds(now),
		...seedContacts(now),
		tags: ['Evaluating', 'Feature', 'Ready', 'Research', 'Visual Bug', 'Working'].map(
			(name, i) => ({
				id: i + 1,
				workspace_id: 1,
				name,
				sort_order: i + 1,
				created_at: now
			})
		),
		tagged: (
			[
				[2, 9],
				[2, 14],
				[2, 17],
				[2, 18],
				[3, 13],
				[3, 15],
				[3, 20],
				[4, 11],
				[4, 17],
				[6, 3]
			] as const
		).map(([tag_id, task_id]) => ({ tag_id, task_id })),
		savedTaskViews: []
	};
}

/** What the daemon titles an article whose page gives no title: its path's last part, else its host. */
/** One page of `rows`, sized as the daemon sizes it: 50 when unasked, never over 500. */
/** A search query's words, lowercased, as the full-text index splits them. */
function searchWords(query: string): string[] {
	return (query.match(/[\p{L}\p{N}]+/gu) ?? []).map((w) => w.toLowerCase());
}

/** Whether `text` has a word starting with the word asked about: the index matches prefixes. */
function starts(text: string): (word: string) => boolean {
	const tokens = text.toLowerCase().match(/[\p{L}\p{N}]+/gu) ?? [];
	return (word) => tokens.some((token) => token.startsWith(word));
}

function paged<T>(rows: T[], offset: number, limit: number | null): { rows: T[]; total: number } {
	const size = Math.min(limit ?? 50, 500);
	return { rows: rows.slice(offset, offset + size), total: rows.length };
}

function fallbackTitle(url: URL): string {
	return url.pathname.split('/').findLast((part) => part !== '') ?? url.host;
}

/** A contact's photo: a 64-pixel PNG of a head and shoulders. */
const CONTACT_PHOTO: Photo = {
	kind: 'image',
	photo_media_type: 'image/png',
	photo_data:
		'iVBORw0KGgoAAAANSUhEUgAAAEAAAABACAIAAAAlC+aJAAACAklEQVR42tXPWVLCUBCF4bM6F+FyXE6QSUAIMgkBAoEwyCguwGVIeTF1DRBC91XSVd9rV/8HlvshGqz+TjSk+jvRkOrtRMNj7100pLvvoskfkHG2oiHrbEVDtrMVDbnORjTk2hvRkG9vRMPT61q0Pxnw6VjnmB9QaK0NikjXGfyIYmttRMx0nZG/+wErPkL9zwbua5SaKyZyvcL8Ln/Ac2PJwaxXOAEoN5YcRgZwAlCuL8mM1B82UBtQqS/IDA4gN8CuLcgMDiA3yB9QfXkjMziA3CB/wEv1jczgAHIDatU5mcEB5AbU7DmZyQHUBtTtOYeRek4AGvacw8gATgAalRkTt573Hc3KjIk5gPkdzfKM6e7+gVy/v2V+R6s8Jdu/D9DqA+QGvD5PCfTfhBnnzgklMFgfZ8bF26sHtEuTq1ws4LuqB53SJL5/qFfiJ6FTnMTxb+m/ZsQIg1P047jJgDhhcAr+RTepP2y41IZuwY92w3olOg+9p3GEm9crEYVIfn30BvTz43MSNeBcJNz8+KRE1SsnO/cDRiclcsCJTgxyo2MJrFeOU+UPGGa9kMTWK6FaeFkvJOEDQrXwMp4u4fWHDVowRpmhTsQAPRjj9FAnYoAeDHH1oQ3wHwcBQQOCZvkDJqlBQNCAoBnTlKsIqldUNqaWq8gb8J2NmeUq4gao7C92itymzsyHpAAAAABJRU5ErkJggg=='
};

/** Three contacts in the first workspace, the first with a photo. */
function seedContacts(now: number): Pick<MockState, 'contacts' | 'contactMethods'> {
	const contact = (
		id: number,
		first_name: string,
		middle_name: string | null,
		last_name: string,
		notes: string,
		photo: Photo = { kind: 'none' }
	) => ({
		id,
		workspace_id: 1,
		deleted: false,
		first_name,
		middle_name,
		last_name,
		notes,
		created_at: now - id * 86_400,
		modified: now - id * 3_600,
		photo
	});
	const method = (id: number, contact_id: number, kind: ContactMethodKind, value: string) => ({
		id,
		contact_id,
		kind,
		value,
		created_at: now - contact_id * 86_400,
		modified: now - contact_id * 86_400
	});
	return {
		contacts: [
			contact(1, 'Mira', null, 'Okafor', 'Neighbour at number 12. Has a spare key.', CONTACT_PHOTO),
			contact(
				2,
				'Tomás',
				'Eduardo',
				'Reyes',
				'Plumber. Fixed the tap in March.\n\nPrefers texts to calls.'
			),
			contact(3, 'Hannah', null, 'Bell', '')
		],
		contactMethods: [
			method(1, 1, 'phone', '+44 7700 900123'),
			method(2, 1, 'email', 'mira.okafor@example.com'),
			method(3, 2, 'phone', '+44 7700 900456'),
			method(4, 3, 'email', 'hannah@bell.example')
		]
	};
}

/** Six articles across the feeds: one read and one whose page couldn't be fetched among them. */
function seedArticles(now: number): Article[] {
	return [
		{
			id: 1,
			deleted: false,
			url: 'https://daringfireball.net/2026/11/touch_and_glow',
			feed_id: 1,
			read: true,
			title: '★ Touch and Glow',
			author: 'John Gruber',
			published_at: now - 5 * DAY,
			published_precision: 'time',
			snapshot: {
				state: 'captured',
				final_url: 'https://daringfireball.net/2026/11/touch_and_glow',
				page_title: 'Daring Fireball: Touch and Glow',
				fetched_at: now - 2 * DAY,
				truncated: false
			},
			contents:
				'Touch and Glow\n\nA week with the new MacBook Pro and its touchscreen.\n\nThe display is the headline, but the surprise is how rarely the touchscreen gets in the way.',
			created_at: now - 2 * DAY,
			modified: now - 2 * DAY
		},
		{
			id: 2,
			deleted: false,
			url: 'https://www.phoronix.com/news/HyprBSD',
			feed_id: 2,
			read: false,
			title:
				'HyprBSD Provides Wayland-Based Desktop Built Atop FreeBSD, Complete With Steam Gaming',
			author: 'Michael Larabel',
			published_at: now - DAY,
			published_precision: 'time',
			snapshot: {
				state: 'failed',
				failure: { kind: 'unreachable', message: 'dns error: failed to lookup address' },
				attempted_at: now - DAY
			},
			contents: '',
			created_at: now - DAY,
			modified: now - DAY
		},
		feedItem(
			3,
			1,
			'John Gruber',
			'https://www.youtube.com/watch?v=f2GZtWuUPl0',
			'Yours Truly on Full Stack Creator, Talking About the New MacBook Pros With PROLED XDR Displays',
			now - 3 * DAY,
			now,
			'A podcast appearance about the new MacBook Pros and their displays.'
		),
		feedItem(
			4,
			2,
			'Michael Larabel',
			'https://www.phoronix.com/news/KDE-Plasma-6.9-Leak-Desktop',
			'KDE Plasma 6.9 Will Stop Leaking Memory When Right Clicking Files/Folders On Desktop',
			now - 6 * DAY,
			now,
			'A fix in Plasma 6.9 stops the desktop leaking memory each time a context menu opens.'
		),
		feedItem(
			5,
			3,
			'John Voorhees',
			'https://www.macstories.net/stories/notes-from-apples-siri-ai-labs-schemas-tools-and-skills/',
			'Notes from Apple’s Siri AI Labs: Schemas, Tools, and Skills',
			now - 2 * DAY,
			now,
			'Notes from a hands-on session on how apps describe their actions to Siri.'
		),
		feedItem(
			6,
			4,
			null,
			'https://thisweek.gnome.org/posts/2026/11/twig-273/',
			'#273 Kanji & Colons',
			now - 4 * DAY,
			now,
			'Updates from the GNOME community this week.'
		)
	];
}

/** A captured, unread article. */
function feedItem(
	id: number,
	feed_id: number,
	author: string | null,
	url: string,
	title: string,
	published_at: number,
	now: number,
	contents: string
): Article {
	return {
		id,
		deleted: false,
		feed_id,
		read: false,
		url,
		title,
		author,
		published_at,
		published_precision: 'time',
		snapshot: {
			state: 'captured',
			final_url: url,
			page_title: title,
			fetched_at: now,
			truncated: false
		},
		contents,
		created_at: now,
		modified: now
	};
}

/** Five feeds, read an hour ago. */
function seedFeeds(now: number): Feed[] {
	return (
		[
			[1, 'https://daringfireball.net/feeds/main', 'Daring Fireball', 2],
			[2, 'https://www.phoronix.com/rss.php', 'Phoronix', 2],
			[3, 'https://www.macstories.net/feed/atom', 'MacStories', 1],
			[4, 'https://thisweek.gnome.org/index.xml', 'This Week in GNOME', 1],
			[5, 'https://simonwillison.net/atom/everything/', 'Simon Willison’s Weblog', 0]
		] as const
	).map(([id, url, title, added]) => ({
		id,
		deleted: false,
		url,
		title,
		sort_order: id,
		full_page_policy: 'whenExcerpt',
		status: { state: 'refreshed', at: now - 3600, added },
		created_at: now - 7 * DAY
	}));
}

/**
 * One Notion account, with its roadmap page imported into the first
 * workspace, and an orphaned Notion account, from before the extension was
 * installed again, whose page stays imported until it moves to another.
 */
function seedConnectorData(
	now: number
): Pick<
	MockState,
	'connectors' | 'definitions' | 'connectorAccounts' | 'resources' | 'shownKinds'
> {
	const connectors = seedConnectors(now);
	const notion = connectors[0];
	const page = notion.resources[0];
	const launch = notion.resources.find((r) => r.identifier === 'page-launch')!;
	return {
		connectors,
		definitions: [
			{
				id: 1,
				identifier: 'com.example.notion',
				version: 1,
				title: notion.title,
				icon: NOTION_ICON,
				unit: notion.unit,
				kinds: [
					{
						id: 1,
						connector_id: 1,
						identifier: 'page',
						title: 'Page',
						plural: 'Pages',
						contents: 'Markdown of the page'
					},
					{
						id: 2,
						connector_id: 1,
						identifier: 'database',
						title: 'Database',
						plural: 'Databases',
						contents: 'JSON { rows: [{ item: string, amount: number }] }'
					}
				],
				metadata_keys: ['author', 'url'],
				created_at: now - 7 * DAY
			}
		],
		connectorAccounts: [
			{
				id: 1,
				connector_id: 1,
				host: {
					kind: 'extension',
					extension_id: notion.extension_id,
					identifier: notion.account.identifier
				},
				description: notion.account.description,
				// Its sign-in lapsed: refreshing the credential failed two hours ago.
				credential_failed_at: now - 7200,
				created_at: now - 7 * DAY
			},
			{
				id: 2,
				connector_id: 1,
				host: { kind: 'orphaned' },
				description: 'Old workspace',
				credential_failed_at: null,
				created_at: now - 9 * DAY
			}
		],
		resources: [
			{
				id: 1,
				workspace_id: 1,
				account_id: 1,
				identifier: page.identifier,
				kind_id: 1,
				kind: `com.example.notion.${page.kind_identifier}`,
				title: page.title,
				content_kind: page.content_kind,
				contents: page.contents,
				// Older than the service's copy, so "Refresh now" updates it.
				last_updated: page.last_updated - DAY,
				modified: now - 3 * DAY
			},
			{
				id: 2,
				workspace_id: 1,
				account_id: 2,
				identifier: 'page-ideas',
				kind_id: 1,
				kind: 'com.example.notion.page',
				title: 'Ideas',
				content_kind: 'markdown',
				contents: '# Ideas\n\nA reading nook.',
				last_updated: now - 9 * DAY,
				modified: now - 9 * DAY
			},
			// No modified date at the service (0 is unknown).
			{
				id: 3,
				workspace_id: 1,
				account_id: 1,
				identifier: 'page-playlist',
				kind_id: 1,
				kind: 'com.example.notion.page',
				title: 'Road trip playlist',
				content_kind: 'markdown',
				contents: 'Tracks to drive to.',
				last_updated: 0,
				modified: now - DAY
			},
			// Its connector draws it, so it shows blocks rather than its contents.
			{
				id: 4,
				workspace_id: 1,
				account_id: 1,
				identifier: launch.identifier,
				kind_id: 1,
				kind: `com.example.notion.${launch.kind_identifier}`,
				title: launch.title,
				content_kind: launch.content_kind,
				contents: launch.contents,
				last_updated: launch.last_updated,
				modified: now - DAY
			}
		],
		// Pages show in the first workspace; databases are hidden.
		shownKinds: [{ id: 1, workspace_id: 1, kind_id: 1, sort_order: 1 }]
	};
}

/** What follows `Context rebuilt` for each reason, as the daemon words it. */
const rebuildBecause: Record<ContextRebuildReason | 'none', string> = {
	cacheExpired: ' because provider cache expired',
	contextLimit: ' because context reached its size limit',
	memoryCapacityChanged: ' because memory capacity changed',
	modelChanged: ' because the default model changed',
	noThread: ' because there was no live thread',
	turnFailed: ' because the last turn failed',
	none: ''
};

/**
 * A message in one line (what follows its verb, for one that has one), as the daemon's `IntelligenceAgentMessage::EXCERPT_SQL`
 * words it, so mock mode can stand in for the daemon.
 */
function agentExcerpt(content: AgentMessageContent, attachment_count: number): string {
	const line = (() => {
		switch (content.kind) {
			case 'text':
				return content.text;
			case 'forward':
				return content.note.trim() || content.item.title;
			case 'choice':
				return content.question;
			case 'codeExecution':
				return content.title;
			case 'documentation':
				return `Learning about ${content.module}`;
			case 'memory':
				return content.memory;
			case 'askedAgent':
				return `Asked ${content.agent.name}: ${content.question}`;
			case 'askedByAgent':
				return `${content.agent.name} asked: ${content.question}`;
			case 'error':
				return content.error;
			case 'note':
				return content.note;
			case 'rebuiltContext':
				return `Context rebuilt${rebuildBecause[content.reason ?? 'none']}`;
			case 'woke':
				switch (content.by.kind) {
					case 'cue':
						return `Cue “${content.by.cue_name}” fired`;
					case 'actorSession':
						return `Actor session ${content.by.actor_session_id} stopped`;
					case 'assignedTask':
						return `“${content.by.title}” was assigned to you`;
				}
		}
	})()
		.replace(/\*\*|__|`/g, '')
		.replace(/[\t\n\r]/g, ' ')
		.replace(/ {2}/g, ' ')
		.trim();
	if (line === '' && attachment_count > 0)
		return attachment_count === 1 ? '1 image' : `${attachment_count} images`;
	return line.length > 200 ? `${line.slice(0, 200)}…` : line;
}

/** What a message is to the fork that posted it, as `RUN_PART_SQL` says. */
function agentRunPart(content: AgentMessageContent, run_id: number | null): AgentRunPart | null {
	if (run_id === null) return null;
	switch (content.kind) {
		case 'woke':
		case 'askedByAgent':
			return 'trigger';
		case 'codeExecution':
		case 'documentation':
		case 'memory':
		case 'note':
		case 'askedAgent':
			return 'step';
		default:
			return 'surfaced';
	}
}

/** What the agent did, for a line of its bookkeeping, as `EXCERPT_VERB_SQL` words it. */
function agentExcerptVerb(content: AgentMessageContent): string | null {
	switch (content.kind) {
		case 'memory':
			return content.change === 'forgotten' ? 'Forgot' : 'Remembered';
		case 'note':
			return 'Noted';
		case 'woke':
			return 'Woke';
		default:
			return null;
	}
}

function agentMessage(
	id: number,
	agent_id: number,
	sent_at: number,
	sender: IntelligenceAgentMessage['sender'],
	content:
		| string
		| AgentMessageContent
		| { execution: AgentCodeExecution }
		| { choice: AgentChoice }
		| { memory: string; change: AgentMemoryChange },
	extra: Partial<IntelligenceAgentMessage> = {}
): IntelligenceAgentMessage {
	const message_content: AgentMessageContent =
		typeof content === 'string'
			? { kind: 'text', text: content }
			: 'kind' in content
				? content
				: 'execution' in content
					? { kind: 'codeExecution', ...content.execution }
					: 'choice' in content
						? { kind: 'choice', ...content.choice }
						: { kind: 'memory', ...content };
	const message = {
		id,
		agent_id,
		run_id: null,
		run_description: null,
		sent_at,
		sender,
		message_content,
		reaction: null,
		reacted_at: null,
		attachment_count: 0,
		reply_to: null,
		...extra
	};
	const excerpt = agentExcerpt(message_content, message.attachment_count);
	return {
		...message,
		run_part: agentRunPart(message_content, message.run_id),
		excerpt,
		excerpt_verb: agentExcerptVerb(message_content),
		...reactionExcerpt(message.reaction, excerpt)
	};
}

/** The line about a message's reaction, as `REACTION_EXCERPT_SQL` words it. */
function reactionExcerpt(
	reaction: string | null,
	excerpt: string
): Pick<IntelligenceAgentMessage, 'reaction_excerpt' | 'reaction_excerpt_verb'> {
	if (reaction === null) return { reaction_excerpt: null, reaction_excerpt_verb: null };
	const quoted = excerpt.length > 140 ? `${excerpt.slice(0, 140)}…` : excerpt;
	return { reaction_excerpt: `${reaction} to “${quoted}”`, reaction_excerpt_verb: 'Reacted' };
}

/**
 * Two actors on the MBP, Claude Code in two checkouts. The first has
 * a task paused on a question, after reading and a tool use, one that
 * ended and one working; the second has none. Their activities are the
 * stretches of that work: the working task's running, the paused one's
 * paused, and three that ended. The working one launched three subagents:
 * one done, with one of its own, and one in the background waiting on a
 * permission.
 */
function seedActors(
	now: number
): Pick<MockState, 'actors' | 'actorSessions' | 'actorSessionMessages' | 'activities'> {
	const actor = (id: number, identifier: string, scope: string): Actor => ({
		id,
		deleted: false,
		client_id: 2,
		identifier,
		product: 'Claude Code',
		scope,
		modes: [
			{
				identifier: 'auto',
				title: 'Auto',
				description: 'A classifier approves or denies each action; asks only when unsure.'
			},
			{
				identifier: 'default',
				title: 'Ask',
				description: 'Asks before editing files or running commands.'
			},
			{
				identifier: 'plan',
				title: 'Plan',
				description: 'Plans first, and changes nothing until you approve.'
			}
		],
		models: [
			{
				identifier: 'opus',
				title: 'Opus 5.5',
				efforts: [
					{ identifier: 'medium', title: 'Medium' },
					{ identifier: 'high', title: 'High' }
				]
			},
			{ identifier: 'haiku', title: 'Haiku 5.5', efforts: [] }
		],
		title: 'Claude Code on MBP'
	});
	const message = (
		id: number,
		session_id: number,
		sent_at: number,
		content: ActorSessionMessageContent,
		answered = false
	): ActorSessionMessage => ({ id, session_id, sent_at, content, answered, attachment_count: 0 });
	const said = (content: ActorMessageContent): ActorSessionMessageContent => ({
		sender: 'actor',
		...content
	});
	return {
		actors: [
			actor(1, 'cohesion', '/Users/me/Developer/cohesion'),
			actor(2, 'cohesion-plus', '/Users/me/Developer/cohesion-plus')
		],
		actorSessions: [
			{
				id: 1,
				actor_id: 1,
				title: 'Install extension from URL',
				mode: null,
				model: 'opus',
				effort: 'high',
				started_at: now - 600,
				status_changed_at: now - 60,
				status: 'needsAttention',
				unread: false,
				read_at: null,
				context: { used_tokens: 41_800, total_tokens: 200_000 },
				follower: { agent_id: 1, initiative: 'user' },
				subagent: null
			},
			{
				id: 2,
				actor_id: 1,
				title: 'CI for the consolidated repos',
				mode: 'auto',
				model: null,
				effort: null,
				started_at: now - DAY,
				status_changed_at: now - DAY + 900,
				status: 'idle',
				unread: true,
				read_at: null,
				context: { used_tokens: 118_400, total_tokens: 200_000 },
				follower: { agent_id: 1, initiative: 'user' },
				subagent: null
			},
			{
				id: 3,
				actor_id: 1,
				title: 'Web UI design hill-climb',
				mode: null,
				model: null,
				effort: null,
				started_at: now - 4 * 60,
				status_changed_at: now - 4 * 60,
				status: 'working',
				unread: false,
				read_at: null,
				context: null,
				follower: { agent_id: 1, initiative: 'user' },
				subagent: null
			},
			{
				id: 4,
				actor_id: 2,
				title: 'Server version in Settings › About',
				mode: null,
				model: null,
				effort: null,
				started_at: now - 2 * 60 * 60,
				status_changed_at: now - 60 * 60,
				status: 'failed',
				unread: false,
				read_at: null,
				context: null,
				follower: { agent_id: 1, initiative: 'user' },
				subagent: null
			},
			{
				id: 5,
				actor_id: 1,
				title: 'Rename and archive the old repos, push the fresh core repo',
				mode: null,
				model: null,
				effort: null,
				started_at: now - 2 * DAY,
				status_changed_at: now - DAY,
				status: 'resolved',
				unread: false,
				read_at: null,
				context: null,
				follower: { agent_id: 1, initiative: 'user' },
				subagent: null
			},
			// Three subagents of “Web UI design hill-climb”: one done, with one
			// of its own, and one in the background waiting on a permission.
			{
				id: 6,
				actor_id: 1,
				title: 'Survey the sidebar’s spacing',
				mode: null,
				model: 'haiku',
				effort: null,
				started_at: now - 3 * 60 - 40,
				status_changed_at: now - 3 * 60,
				status: 'idle',
				unread: false,
				read_at: null,
				context: null,
				follower: null,
				subagent: {
					parent_session_id: 3,
					key: 'agent-survey',
					agent_type: 'Explore',
					message_id: 11,
					background: false
				}
			},
			{
				id: 7,
				actor_id: 1,
				title: 'Run the visual checks',
				mode: null,
				model: null,
				effort: null,
				started_at: now - 2 * 60 - 50,
				status_changed_at: now - 30,
				status: 'needsAttention',
				unread: false,
				read_at: null,
				context: null,
				follower: null,
				subagent: {
					parent_session_id: 3,
					key: 'agent-checks',
					agent_type: 'general-purpose',
					message_id: 12,
					background: true
				}
			},
			{
				id: 8,
				actor_id: 1,
				title: 'Read the spacing tokens',
				mode: null,
				model: 'haiku',
				effort: null,
				started_at: now - 3 * 60 - 30,
				status_changed_at: now - 3 * 60 - 10,
				status: 'idle',
				unread: false,
				read_at: null,
				context: null,
				follower: null,
				subagent: {
					parent_session_id: 6,
					key: 'agent-tokens',
					agent_type: 'Explore',
					message_id: 16,
					background: false
				}
			}
		],
		activities: [
			{
				id: 1,
				title: 'Web UI design hill-climb',
				kind: { kind: 'actorSession', actor_session_id: 3 },
				status: 'running',
				progress: 'Taking screenshots in mock mode',
				started_at: now - 4 * 60,
				ended_at: null
			},
			{
				id: 2,
				title: 'Install extension from URL',
				kind: { kind: 'actorSession', actor_session_id: 1 },
				status: 'paused',
				progress: 'Asked how to review the script',
				started_at: now - 600,
				ended_at: null
			},
			{
				id: 3,
				title: 'CI for the consolidated repos',
				kind: { kind: 'actorSession', actor_session_id: 2 },
				status: 'succeeded',
				progress: null,
				started_at: now - DAY,
				ended_at: now - DAY + 900
			},
			{
				id: 4,
				title: 'CI for the consolidated repos',
				kind: { kind: 'actorSession', actor_session_id: 2 },
				status: 'failed',
				progress: null,
				started_at: now - 2 * DAY,
				ended_at: now - 2 * DAY + 300
			},
			{
				id: 5,
				title: 'Install extension from URL',
				kind: { kind: 'actorSession', actor_session_id: 1 },
				status: 'cancelled',
				progress: null,
				started_at: now - 3 * DAY,
				ended_at: now - 3 * DAY + 900
			}
		],
		actorSessionMessages: [
			message(1, 1, now - 600, {
				sender: 'user',
				kind: 'text',
				text: 'Install extension from URL'
			}),
			message(
				2,
				1,
				now - 590,
				said({ kind: 'text', text: 'On it. I’ll read the extensions page first.' })
			),
			message(
				8,
				1,
				now - 500,
				said({
					kind: 'thinking',
					text: 'Installing from a file already **reviews the real manifest** before anything runs. A URL only changes where the script comes from, so the download should land in that same review; start with `install_extension.rs` and how it reads a file.',
					ended_at: now - 488
				})
			),
			message(
				3,
				1,
				now - 400,
				said({
					kind: 'toolUse',
					tool: 'Read',
					category: 'read',
					summary: 'cohesiond/src/routes/install_extension.rs',
					ended_at: now - 396
				})
			),
			message(
				4,
				1,
				now - 60,
				said({
					kind: 'question',
					questions: [
						{
							header: 'Approach',
							question: 'How should a script from a URL be reviewed?',
							options: [
								{ label: 'Same review', description: 'Download it, then open the file review.' },
								{
									label: 'Trust the registry',
									description: 'Skip the review when a registry lists the URL.'
								}
							],
							multi_select: false,
							allows_other: true
						}
					]
				})
			),
			message(5, 2, now - DAY, {
				sender: 'user',
				kind: 'text',
				text: 'CI for the consolidated repos'
			}),
			message(
				7,
				2,
				now - DAY + 600,
				said({
					kind: 'toolUse',
					tool: 'Bash',
					category: 'command',
					summary: 'gh run list --limit 5',
					ended_at: now - DAY + 840
				})
			),
			message(
				6,
				2,
				now - DAY + 900,
				said({
					kind: 'text',
					text: 'Done. CI is green in every repo. The crates it builds:\n\n```mermaid\ngraph TD\n  cohesiond --> cohesionkit\n  cohesionkit --> rusqlite\n  cohesionkit --> rquickjs\n```'
				})
			),
			message(9, 3, now - 4 * 60, {
				sender: 'user',
				kind: 'text',
				text: 'Web UI design hill-climb'
			}),
			message(
				10,
				3,
				now - 4 * 60 + 5,
				said({
					kind: 'text',
					text: 'I’ll survey the sidebar first, and run the visual checks alongside.'
				})
			),
			message(
				11,
				3,
				now - 3 * 60 - 40,
				said({
					kind: 'toolUse',
					tool: 'Task',
					category: 'agent',
					summary: 'Survey the sidebar’s spacing',
					ended_at: now - 3 * 60
				})
			),
			message(
				12,
				3,
				now - 2 * 60 - 50,
				said({
					kind: 'toolUse',
					tool: 'Task',
					category: 'agent',
					summary: 'Run the visual checks',
					ended_at: now - 2 * 60 - 49
				})
			),
			message(
				13,
				3,
				now - 2 * 60,
				said({
					kind: 'toolUse',
					tool: 'Edit',
					category: 'edit',
					summary: 'web/src/lib/shell/Sidebar.svelte',
					ended_at: null
				})
			),
			message(
				14,
				6,
				now - 3 * 60 - 39,
				said({
					kind: 'thinking',
					text: 'Start with the sidebar’s rows, then the tokens they use.',
					ended_at: now - 3 * 60 - 36
				})
			),
			message(
				15,
				6,
				now - 3 * 60 - 35,
				said({
					kind: 'toolUse',
					tool: 'Read',
					category: 'read',
					summary: 'web/src/lib/shell/Sidebar.svelte',
					ended_at: now - 3 * 60 - 33
				})
			),
			message(
				16,
				6,
				now - 3 * 60 - 31,
				said({
					kind: 'toolUse',
					tool: 'Task',
					category: 'agent',
					summary: 'Read the spacing tokens',
					ended_at: now - 3 * 60 - 10
				})
			),
			message(
				17,
				6,
				now - 3 * 60 - 2,
				said({
					kind: 'text',
					text: 'Rows sit 2px apart while groups sit 12px apart, so the gap between groups reads as a break. `--s3` would even it out.'
				})
			),
			message(
				18,
				8,
				now - 3 * 60 - 29,
				said({
					kind: 'toolUse',
					tool: 'Read',
					category: 'read',
					summary: 'web/src/lib/tokens.css',
					ended_at: now - 3 * 60 - 27
				})
			),
			message(
				19,
				8,
				now - 3 * 60 - 12,
				said({ kind: 'text', text: 'The spacing scale runs `--s1` (2px) to `--s8` (48px).' })
			),
			message(
				20,
				7,
				now - 2 * 60 - 48,
				said({
					kind: 'toolUse',
					tool: 'Bash',
					category: 'command',
					summary: 'npm run build',
					ended_at: now - 60
				})
			),
			message(
				21,
				7,
				now - 30,
				said({
					kind: 'permission',
					subject: {
						kind: 'command',
						command: 'npx playwright test --update-snapshots',
						cwd: '/Users/me/Developer/cohesion/web'
					},
					reason: 'To refresh the screenshots the sidebar change moved.',
					decisions: ['allow', 'allowForSession', 'deny']
				})
			)
		]
	};
}

/**
 * Two teams: Cohesion Engineers, with Core Engineer (pinned as its lead) and Plus Engineer,
 * and Assistants, with Secretary and Researcher.
 * Core Engineer, with no cues now, has a feed with a short chat
 * (each side reacting to the other),
 * a note and an answered question from runs on an old launch cue, and a
 * question waiting on a pick;
 * Secretary, with a task-created and a priority cue, has a feed with an
 * exchange, a code run, a run's note, a
 * question turned down, two answered, one waiting, and a
 * request it remembered. The memories are one global, one of Secretary's and one of Assistants'.
 */
/** A bar chart of five days, the image Core Engineer shows. */
const WEEK_CHART =
	'iVBORw0KGgoAAAANSUhEUgAAAPAAAACMCAIAAADN17N/AAAB5UlEQVR42u3dwQ1AERRFQR3qRE3WCsRSFYRnklPAjTfLn/zU+5TClDyBgJaAloCWgBbQEtAS0BLQEtACWgJaAloCWgJaQEtAS0BLQEtABy+XsSmgBTTQAhpoAQ000EALaKAFNNACGmgBDTTQQAtooAU00AIaaKCBBhpooAU00AIaaAENNNBAC2igBfR9G4AGGmgBDbSABlpAAw000AIaaAENtIAGWkADDTTQAhpoAQ20gAZaQIcH7TtgoIEGGmiH9A5AAw000EADDTTQQAMNNNAO6R2ABhpooIEGGmiggQYaaKAd0jsADTTQQAMNNNBAAw000EA7pHcAGmiggQYaaKB/OqQNQMMENNA2AA20DUA7pA1AwwQ00DABDbQNQANtA9AOaQPQMAENNExAA20D0EDbALRD2nAAtEe0AWiHtAFoG2wA2gaggbYBaIe0AWiHtAFoG2wA2gagPaINQDukDUA7pA1A22AD0DYA7RFtANohbQDaIW0A2gYbgLYBaI9oA9AOaQPQDmkD0DbYALQNQHtEG4B2SBuAdkgbgLbBBqBtANoj2gC0Q9oAtA02AG0D0EDbALRHtAFoh7QBaBtsANoGoIG2AWiPaMNLoGtt0lUF/5OsBLSAloCWgJaAloAW0BLQ0qkWqfkkEnJUAs8AAAAASUVORK5CYII=';

function seedAgents(
	now: number
): Pick<
	MockState,
	| 'agents'
	| 'agentTeams'
	| 'agentMessages'
	| 'agentAttachments'
	| 'agentCues'
	| 'agentRuns'
	| 'memories'
> {
	const hour = 3600;
	return {
		agents: [
			{
				id: 1,
				created_at: now - 5 * DAY,
				agent_name: 'Core Engineer',
				agent_color: 'blue',
				agent_emoji: '💻',
				agent_personality: 'professional',
				agent_verbosity: 'terse',
				agent_role: '',
				agent_directive: '',
				activity: null,
				foreground_started: null,
				context_tokens: 18400,
				agent_unread: false,
				last_message_at: null,
				pinned: true,
				sort_order: 1,
				team_id: 1,
				following_count: 0
			},
			{
				id: 2,
				created_at: now - 3 * DAY,
				agent_name: 'Secretary',
				agent_color: 'pink',
				agent_emoji: null,
				agent_personality: 'friendly',
				agent_verbosity: 'detailed',
				agent_role: 'Files new inbox tasks',
				agent_directive: 'Never move a task out of Church. Ask before archiving anything.',
				activity: null,
				foreground_started: null,
				// Its thread's summary held nothing: one rule shows.
				context_tokens: null,
				agent_unread: true,
				// An unread agent has posted, so it has a time to show.
				last_message_at: now - 25 * 60,
				pinned: false,
				sort_order: 2,
				team_id: 2,
				following_count: 0
			},
			{
				id: 3,
				created_at: now - 4 * DAY,
				agent_name: 'Plus Engineer',
				agent_color: 'green',
				agent_emoji: '📱',
				agent_personality: 'balanced',
				agent_verbosity: 'brief',
				agent_role: 'Cohesion Plus Engineer',
				agent_directive: 'You report to the Core Engineer.',
				activity: null,
				foreground_started: null,
				context_tokens: 9600,
				agent_unread: false,
				last_message_at: null,
				pinned: false,
				sort_order: 3,
				team_id: 1,
				following_count: 0
			},
			{
				id: 4,
				created_at: now - 2 * DAY,
				agent_name: 'Researcher',
				agent_color: 'purple',
				agent_emoji: '📰',
				agent_personality: 'balanced',
				agent_verbosity: 'detailed',
				agent_role: 'News Researcher',
				agent_directive: '',
				activity: null,
				foreground_started: null,
				context_tokens: null,
				agent_unread: false,
				last_message_at: null,
				pinned: false,
				sort_order: 4,
				team_id: 2,
				following_count: 0
			}
		],
		agentTeams: [
			{ id: 1, title: 'Cohesion Engineers' },
			{ id: 2, title: 'Assistants' }
		],
		agentMessages: [
			agentMessage(1, 1, now - 5 * DAY, 'user', 'What’s open in Core?', {
				reaction: '👋',
				reacted_at: now - 5 * DAY + 4
			}),
			agentMessage(
				2,
				1,
				now - 5 * DAY + 4,
				'agent',
				'Three tasks: **Repeating tasks**, **Subagent sessions**, and **Add a command to manually rebuild an agent’s context**, which is assigned to me.',
				{ reaction: '👍', reacted_at: now - 5 * DAY + 30 }
			),
			agentMessage(
				3,
				2,
				now - 3 * DAY,
				'user',
				'Whenever I add a task to the Cohesion inbox, file it into the right folder.',
				{ reaction: '👍', reacted_at: now - 3 * DAY + 10 }
			),
			agentMessage(4, 2, now - 3 * DAY + 3, 'agent', {
				execution: {
					title: 'Listing the Cohesion task folders',
					code: 'cohesion.workspace(1).taskFolders().map((f) => f.title)',
					outcome: {
						status: 'succeeded',
						result: '["Core","WebUI","Plus","Cloud","JavaScript","WebUI Harnesses"]'
					}
				}
			}),
			agentMessage(
				5,
				2,
				now - 3 * DAY + 6,
				'agent',
				'You have **Core**, **WebUI**, **Plus**, **Cloud**, **JavaScript** and **WebUI Harnesses**. I’ll watch for new inbox tasks and propose a folder for each one; nothing moves until you approve.'
			),
			agentMessage(
				8,
				2,
				now - 2 * hour,
				'backgroundAgent',
				'“Configure and deploy the Linear connector” fits Cloud and Core equally, so I left it in the inbox.',
				{ run_id: 1, run_description: '“Configure and deploy the Linear connector” was created' }
			),
			agentMessage(
				9,
				2,
				now - 90 * 60,
				'backgroundAgent',
				{
					choice: {
						question: 'Move “Cohesion-bench” to Core? It scores models on the daemon’s tools.',
						options: ['Yes, move it', 'No'],
						context: 'Task 2; move it with cohesion.workspace(1).task(2).moveTo(1).',
						selected: 1
					}
				},
				{ run_id: 2, run_description: '“Cohesion-bench” was created' }
			),
			agentMessage(
				10,
				2,
				now - 20 * 60,
				'backgroundAgent',
				{
					choice: {
						question:
							'Move “Add a command to manually rebuild an agent’s context” to Core? It’s a daemon command, and you file those under Core.',
						options: ['Yes, move it', 'No'],
						context: 'Task 3; move it with cohesion.workspace(1).task(3).moveTo(1).',
						selected: null
					}
				},
				{
					run_id: 3,
					run_description: '“Add a command to manually rebuild an agent’s context” was created'
				}
			),
			agentMessage(
				11,
				1,
				now - 4 * DAY,
				'backgroundAgent',
				'Morning. Nothing is overdue, and **3 tasks** are due this week.',
				{ run_id: 4, run_description: 'Cohesion launched' }
			),
			agentMessage(
				12,
				1,
				now - 2 * DAY,
				'backgroundAgent',
				{
					choice: {
						question: 'Mark “Install extension from URL” done? Today’s journal says it was pushed.',
						options: ['Yes', 'No'],
						context: 'Task 4; set it done with cohesion.workspace(1).task(4).setStatus("done").',
						selected: 0
					}
				},
				{ run_id: 5, run_description: 'Cohesion launched' }
			),
			agentMessage(
				13,
				2,
				now - 10 * 60,
				'backgroundAgent',
				{
					choice: {
						question: 'Move “Themes/flair” to WebUI? It’s about how the web UI looks.',
						options: ['Yes, move it', 'No'],
						context: 'Task 5; move it with cohesion.workspace(1).task(5).moveTo(2).',
						selected: 0
					}
				},
				{ run_id: 6, run_description: '“Themes/flair” was created' }
			),
			agentMessage(14, 1, now - DAY, 'agent', {
				choice: {
					question: 'Which workspace should I keep an eye on?',
					options: ['Cohesion', 'Church', 'Both'],
					context: null,
					selected: null
				}
			}),
			agentMessage(15, 2, now - 5 * 60, 'user', 'Daemon commands go under Core too, like routes.', {
				reaction: '👍',
				reacted_at: now - 5 * 60 + 3
			}),
			// Two days on, the turn rebuilt the thread from a summary.
			agentMessage(16, 2, now - 5 * 60 + 1, 'agent', {
				kind: 'rebuiltContext',
				reason: 'cacheExpired'
			}),
			agentMessage(30, 2, now - 5 * 60 + 3, 'agent', {
				memory: 'Daemon commands and routes go under Core.',
				change: 'remembered'
			}),
			agentMessage(31, 2, now - 5 * 60 + 4, 'agent', {
				memory: 'Daemon commands go under Plus.',
				change: 'forgotten'
			}),
			agentMessage(
				17,
				1,
				now - 3 * hour,
				'user',
				'Let me know when “Install extension from URL” is done.'
			),
			agentMessage(18, 1, now - 3 * hour + 4, 'agent', 'Will do.'),
			// A fork an actor session woke later, answering the request it follows up on.
			agentMessage(
				19,
				1,
				now - hour,
				'agent',
				'“Install extension from URL” is done and pushed. Want me to add it to the Parity doc?',
				{ reply_to: 17 }
			),
			// A forward with no note, which the agent reacted to after replying.
			agentMessage(
				20,
				1,
				now - 30 * 60,
				'user',
				{
					kind: 'forward',
					note: '',
					item: {
						kind: 'task',
						workspace_id: 1,
						id: 1,
						title: 'Configure and deploy the Linear connector'
					}
				},
				{ reaction: '👍', reacted_at: now - 10 * 60 }
			),
			agentMessage(
				21,
				1,
				now - 30 * 60 + 3,
				'agent',
				'I’ll check on it once the OAuth app is registered.'
			),
			// A forwarded document, and a reply linking one item of each kind.
			agentMessage(24, 1, now - 25 * 60, 'user', {
				kind: 'forward',
				note: 'Anything to add before the launch?',
				item: { kind: 'document', workspace_id: 1, id: 6, title: 'Extensions Registry' }
			}),
			agentMessage(
				25,
				1,
				now - 25 * 60 + 4,
				'agent',
				'It matches [Install extension from URL](cohesion://task/1/4); see [Cohesion-bench](cohesion://task/1/2) too. Rewind landed in [your journal](cohesion://document/1/4), [Secretary](cohesion://agent/2) files the tasks, and [the install session](cohesion://actor-session/1) is still waiting on you.'
			),
			// Core Engineer asks Secretary, whose fork answers.
			agentMessage(22, 1, now - 20 * 60, 'agent', {
				kind: 'askedAgent',
				agent: { id: 2, name: 'Secretary' },
				question:
					'Where do daemon commands go? “Add a command to manually rebuild an agent’s context” is in the inbox.',
				reply: { status: 'answered', text: 'Under **Core**, with routes.' }
			}),
			agentMessage(
				23,
				2,
				now - 20 * 60,
				'backgroundAgent',
				{
					kind: 'askedByAgent',
					agent: { id: 1, name: 'Core Engineer' },
					question:
						'Where do daemon commands go? “Add a command to manually rebuild an agent’s context” is in the inbox.',
					reply: { status: 'answered', text: 'Under **Core**, with routes.' }
				},
				{ run_id: 7 }
			),
			// An image the agent showed, from a script.
			agentMessage(26, 1, now - 10 * 60, 'agent', 'Here’s what you finished each day this week.', {
				attachment_count: 1
			}),
			// A fork an actor session woke, saying why, then a note for its later turns.
			agentMessage(
				27,
				1,
				now - 5 * 60,
				'agent',
				{
					kind: 'woke',
					by: {
						kind: 'actorSession',
						actor_session_id: 2,
						title: 'CI for the consolidated repos',
						stop: 'finished'
					}
				},
				{ run_id: 8 }
			),
			agentMessage(
				28,
				1,
				now - 5 * 60 + 20,
				'agent',
				{
					kind: 'note',
					note: 'CI is green in five repos; apple and webshell have no macOS runners. Next: dispatch the Image workflow fix once the package permission is sorted.'
				},
				{ run_id: 8 }
			),
			// A long answer: a list, a table, code and a diagram in one reply.
			agentMessage(32, 1, now - 4 * 60, 'user', 'How does a release get out?'),
			agentMessage(
				33,
				1,
				now - 4 * 60 + 5,
				'agent',
				'A release goes through four steps, and each one stops the next if it fails. The checks run on your own machine, because there’s nothing else to run them.\n\n## Steps\n\n1. Run the checks.\n2. Build the web bundle.\n3. Install the server.\n4. Restart the service.\n\n| Step | Command | Takes |\n| --- | --- | --- |\n| Checks | `cargo clippy --workspace` | 2 min |\n| Bundle | `npm run build` | 30 sec |\n| Install | `scripts/install-server.sh` | 1 min |\n\n```sh\ncargo fmt --all\ncargo clippy --workspace --all-targets --locked -- -D warnings\ncargo test --workspace --locked\n```\n\n```mermaid\nflowchart LR\n  Commit --> Check[fmt, clippy, tests]\n  Check --> Build[Web bundle]\n  Build --> Install[install-server.sh]\n  Install --> Restart[systemd restart]\n```\n\nThe restart is the only step that interrupts anything, so leave it until nothing is running.'
			),
			// The shortest reply there is, after a tool row and after a question.
			agentMessage(34, 1, now - 3 * 60, 'user', 'Bump the patch version.'),
			agentMessage(35, 1, now - 3 * 60 + 2, 'agent', {
				execution: {
					title: 'Bumping the patch version',
					code: 'cohesion.workspace(1).task(7).setStatus("done")',
					outcome: { status: 'succeeded', result: 'null' }
				}
			}),
			agentMessage(36, 1, now - 3 * 60 + 4, 'agent', 'Done.'),
			agentMessage(37, 1, now - 2 * 60, 'user', 'Thanks.'),
			agentMessage(38, 1, now - 2 * 60 + 2, 'agent', 'Anytime.'),
			// A cue's fork: what it did collapses under what woke it, while what it
			// told the user, its question and its reaction to the user's message stay.
			agentMessage(41, 1, now - 115, 'user', 'Keep an eye on the Linear connector for me.', {
				reaction: '👍',
				reacted_at: now - 92
			}),
			agentMessage(
				42,
				1,
				now - 110,
				'agent',
				{ kind: 'woke', by: { kind: 'cue', cue_name: 'Morning sweep', hook: 'timer' } },
				{ run_id: 10, run_description: 'The time reached 8:00 AM' }
			),
			agentMessage(
				43,
				1,
				now - 105,
				'agent',
				{
					execution: {
						title: 'Read the overdue tasks',
						code: 'return tasks.list({ overdue: true }).map((t) => t.title);',
						outcome: {
							status: 'succeeded',
							result:
								'["Configure and deploy the Linear connector","Persist research text when going back"]'
						}
					}
				},
				{ run_id: 10 }
			),
			agentMessage(
				44,
				1,
				now - 100,
				'agent',
				{ kind: 'documentation', module: 'network' },
				{ run_id: 10 }
			),
			agentMessage(
				45,
				1,
				now - 95,
				'agent',
				'Two tasks need you this morning: **Configure and deploy the Linear connector** and **Persist research text when going back**.',
				{ run_id: 10, reaction: '🎉', reacted_at: now - 70 }
			),
			agentMessage(
				46,
				1,
				now - 90,
				'agent',
				{ memory: 'The Linear connector waits on its OAuth app.', change: 'remembered' },
				{ run_id: 10 }
			),
			agentMessage(47, 1, now - 85, 'user', 'Thanks. What’s left on Cohesion-bench?'),
			agentMessage(
				48,
				1,
				now - 82,
				'agent',
				{
					kind: 'askedAgent',
					agent: { id: 2, name: 'Secretary' },
					question: 'Should an overdue task move to today, or stay where it is?',
					reply: { status: 'answered', text: 'Move it, and leave its folder alone.' }
				},
				{ run_id: 10 }
			),
			agentMessage(
				49,
				1,
				now - 80,
				'agent',
				{
					choice: {
						question: 'Move **Persist research text when going back** to today?',
						options: ['Yes, today', 'Leave it'],
						context: 'Secretary says to move overdue tasks to today.',
						selected: null
					}
				},
				{ run_id: 10 }
			),
			agentMessage(
				50,
				1,
				now - 75,
				'agent',
				{
					kind: 'note',
					note: 'Asked about moving the research text task. The Linear connector can wait until they answer.'
				},
				{ run_id: 10 }
			),
			// A stack of status lines between a question and its reply: the thread
			// rebuilt, two forks woken (one with a step), a reaction, and a memory.
			agentMessage(52, 1, now - 74, 'user', 'Anything I should know before I head out?', {
				reaction: '👍',
				reacted_at: now - 65
			}),
			agentMessage(53, 1, now - 72, 'agent', { kind: 'rebuiltContext', reason: 'cacheExpired' }),
			agentMessage(
				54,
				1,
				now - 70,
				'agent',
				{ kind: 'woke', by: { kind: 'cue', cue_name: 'Inbox check', hook: 'timer' } },
				{ run_id: 11, run_description: 'The time reached 9:00 AM' }
			),
			agentMessage(
				55,
				1,
				now - 68,
				'agent',
				{
					kind: 'woke',
					by: {
						kind: 'actorSession',
						actor_session_id: 2,
						title: 'CI for the consolidated repos',
						stop: 'finished'
					}
				},
				{ run_id: 12 }
			),
			agentMessage(
				56,
				1,
				now - 67,
				'agent',
				{
					execution: {
						title: 'Read the session’s result',
						code: 'return actors.session(2).result;',
						outcome: { status: 'succeeded', result: '"finished"' }
					}
				},
				{ run_id: 12 }
			),
			agentMessage(57, 1, now - 64, 'agent', {
				memory: 'Deploys wait until no actor session is running.',
				change: 'remembered'
			}),
			agentMessage(58, 1, now - 62, 'agent', 'Nothing urgent. CI finished green.'),
			// One job's run of notes, beside the other jobs in this feed.
			agentMessage(
				39,
				2,
				now - 60,
				'backgroundAgent',
				'Three tasks came in overnight. **Two** went to Core.',
				{ run_id: 9, run_description: 'Cue “Morning sweep” fired' }
			),
			agentMessage(40, 2, now - 60 + 2, 'backgroundAgent', 'The third is still in the inbox.', {
				run_id: 9,
				run_description: 'Cue “Morning sweep” fired'
			}),
			// A step of that run, whose run is from before wake lines: it has no trigger.
			agentMessage(
				51,
				2,
				now - 60 + 3,
				'agent',
				{
					execution: {
						title: 'Read the inbox',
						code: 'return tasks.list({ folder: "Inbox" }).length;',
						outcome: { status: 'succeeded', result: '3' }
					}
				},
				{ run_id: 9, run_description: 'Cue “Morning sweep” fired' }
			)
		],
		agentAttachments: [
			{
				id: 1,
				message: { kind: 'agent', agent_message_id: 26 },
				media_type: 'image/png',
				data: WEEK_CHART
			}
		],
		memories: [
			{
				id: 1,
				scope: { kind: 'global' },
				created_at: now - 5 * DAY,
				memory: 'Don’t send messages about unimportant things.'
			},
			{
				id: 2,
				scope: { kind: 'agent', agent_id: 2 },
				created_at: now - 5 * 60 + 3,
				memory: 'Daemon commands and routes go under Core.'
			},
			{
				id: 3,
				scope: { kind: 'team', team_id: 2 },
				created_at: now - 4 * DAY,
				memory:
					'When the user asks for a task, react 👍 to their message instead of replying, unless more is needed.'
			}
		],
		agentCues: [
			{
				id: 1,
				agent_id: 2,
				name: 'File new inbox tasks',
				hook: 'task:create',
				script: '(task) => task.workspaceId === 1 && task.taskFolderId === null',
				instructions:
					'A task was added to the Cohesion inbox. Pick the folder it belongs in and propose moving it there. If none fits, say so.',
				effort: 'low',
				set_at: now - 3 * 86400
			},
			{
				id: 2,
				agent_id: 2,
				name: 'Flag urgent tasks without a due date',
				hook: 'task:priority',
				script: '(task) => task.priority === TaskPriority.urgent && !task.dueDate',
				instructions:
					'A task became urgent but has no due date. Post a note asking when it is due.',
				effort: 'low',
				set_at: now - 3 * 86400
			}
		],
		agentRuns: [
			{
				id: 1,
				agent_id: 2,
				thread_id: 2,
				hook: 'task:create',
				cue_name: 'File new inbox tasks',
				sender_agent_id: null,
				started_at: now - 2 * hour,
				state: { status: 'completed', ended_at: now - 2 * hour + 9 },
				input_tokens: 4_200,
				output_tokens: 180,
				cache_read_tokens: 0,
				cache_write_tokens: 0,
				cost: 0.0153
			},
			{
				id: 2,
				agent_id: 2,
				thread_id: null,
				hook: 'task:create',
				cue_name: 'File new inbox tasks',
				sender_agent_id: null,
				started_at: now - 95 * 60,
				state: {
					status: 'failed',
					ended_at: now - 95 * 60 + 4,
					error: 'The model endpoint timed out.'
				},
				input_tokens: 3_900,
				output_tokens: 0,
				cache_read_tokens: 0,
				cache_write_tokens: 0,
				cost: null
			},
			{
				id: 3,
				agent_id: 2,
				thread_id: 1,
				hook: 'task:create',
				cue_name: 'File new inbox tasks',
				sender_agent_id: null,
				started_at: now - 21 * 60,
				state: { status: 'completed', ended_at: now - 20 * 60 },
				input_tokens: 5_100,
				output_tokens: 240,
				cache_read_tokens: 0,
				cache_write_tokens: 0,
				cost: 0.0189
			},
			{
				id: 4,
				agent_id: 1,
				thread_id: null,
				hook: 'launch',
				cue_name: null,
				sender_agent_id: null,
				started_at: now - 4 * DAY,
				state: { status: 'completed', ended_at: now - 4 * DAY + 6 },
				input_tokens: 3_000,
				output_tokens: 90,
				cache_read_tokens: 0,
				cache_write_tokens: 0,
				cost: 0.0049
			},
			{
				id: 5,
				agent_id: 1,
				thread_id: null,
				hook: 'launch',
				cue_name: null,
				sender_agent_id: null,
				started_at: now - 2 * DAY,
				state: { status: 'completed', ended_at: now - 2 * DAY + 7 },
				input_tokens: 3_400,
				output_tokens: 120,
				cache_read_tokens: 0,
				cache_write_tokens: 0,
				cost: 0.0057
			},
			{
				id: 6,
				agent_id: 2,
				thread_id: null,
				hook: 'task:create',
				cue_name: 'File new inbox tasks',
				sender_agent_id: null,
				started_at: now - 11 * 60,
				state: { status: 'completed', ended_at: now - 10 * 60 },
				input_tokens: 4_400,
				output_tokens: 160,
				cache_read_tokens: 0,
				cache_write_tokens: 0,
				cost: 0.0158
			},
			// A fork of Secretary's conversation, answering Core Engineer's question.
			{
				id: 7,
				agent_id: 2,
				thread_id: null,
				hook: 'agent:message',
				cue_name: null,
				sender_agent_id: 1,
				started_at: now - 20 * 60,
				state: { status: 'completed', ended_at: now - 20 * 60 + 12 },
				input_tokens: 6_100,
				output_tokens: 140,
				cache_read_tokens: 5_200,
				cache_write_tokens: 0,
				cost: 0.0071
			},
			// A fork of Core Engineer's conversation that its morning cue woke.
			{
				id: 10,
				agent_id: 1,
				thread_id: null,
				hook: 'timer',
				cue_name: 'Morning sweep',
				sender_agent_id: null,
				started_at: now - 110,
				state: { status: 'completed', ended_at: now - 74 },
				input_tokens: 5_200,
				output_tokens: 240,
				cache_read_tokens: 4_800,
				cache_write_tokens: 0,
				cost: 0.0062
			},
			// A fork of Core Engineer's conversation that a finished actor session woke.
			{
				id: 8,
				agent_id: 1,
				thread_id: null,
				hook: 'actor:moved',
				cue_name: null,
				sender_agent_id: null,
				started_at: now - 5 * 60,
				state: { status: 'completed', ended_at: now - 5 * 60 + 30 },
				input_tokens: 5_800,
				output_tokens: 90,
				cache_read_tokens: 5_000,
				cache_write_tokens: 0,
				cost: 0.004
			}
		]
	};
}

type Handlers = { [K in RequestType]: (args: Requests[K]) => Replies[K] };

export interface Mock {
	send: Send;
	sendRaw: (request: RawRequest) => Promise<ReplyFrame>;
	state: MockState;
}

const PRIORITY_RANK: Record<TaskPriority, number> = {
	none: 0,
	low: 1,
	medium: 2,
	high: 3,
	urgent: 4
};

// The daemon's task order: ties by id, and tasks missing a size or date last either way.
function compareTasks(a: Task, b: Task, sort: TaskSort, descending: boolean): number {
	const flip = descending ? -1 : 1;
	let order: number;
	if (sort === 'title') {
		order = flip * a.title.toLowerCase().localeCompare(b.title.toLowerCase());
	} else if (sort === 'priority') {
		order = flip * (PRIORITY_RANK[a.priority] - PRIORITY_RANK[b.priority]);
	} else if (sort === 'size') {
		if (a.size === null || b.size === null)
			order = (a.size === null ? 1 : 0) - (b.size === null ? 1 : 0);
		else order = flip * (a.size - b.size);
	} else if (sort === 'startDate' || sort === 'dueDate') {
		const [x, y] = sort === 'startDate' ? [a.start_date, b.start_date] : [a.due_date, b.due_date];
		if (x === null || y === null) order = (x === null ? 1 : 0) - (y === null ? 1 : 0);
		else order = flip * x.localeCompare(y);
	} else {
		order = flip * (a.id - b.id);
	}
	return order || a.id - b.id;
}

/**
 * Rewind's history of workspace 1's first document and first task, by each kind
 * of author, and a deleted item of each kind to bring back.
 */
function seedRewind(state: MockState, now: number): void {
	const MINUTE = 60;
	const d = state.documents.find((d) => d.workspace_id === 1 && d.document_type === 'normal')!;
	const version = (
		id: number,
		ago: number,
		contents: string,
		change: VersionChange,
		author: VersionAuthor,
		author_name: string | null
	): Version => ({
		id,
		item: { kind: 'document', document_id: d.id },
		workspace_id: 1,
		title: d.title,
		body: contents,
		change,
		author,
		author_name,
		created: now - ago,
		modified: now - ago
	});
	state.versions.push(
		version(1, 3 * DAY, 'A first draft.\n', 'created', 'user', null),
		version(2, 2 * DAY, `${d.contents}\n\nTo do: ask Core Engineer.\n`, 'edited', 'user', null),
		version(3, 5 * 60 * MINUTE, 'A rewrite by an agent.\n', 'edited', 'agent', 'Core Engineer'),
		version(4, 90 * MINUTE, 'Synced in from elsewhere.\n', 'edited', 'extension', 'Linear sync'),
		version(5, 12 * MINUTE, d.contents, 'edited', 'user', null)
	);
	const t = state.tasks.find((t) => t.workspace_id === 1)!;
	const taskVersion = (
		id: number,
		ago: number,
		title: string,
		status: TaskStatus,
		change: VersionChange,
		author: VersionAuthor,
		author_name: string | null,
		description = t.description,
		task_id = t.id
	): Version => ({
		id,
		item: { kind: 'task', task_id, status },
		workspace_id: 1,
		title,
		body: description,
		change,
		author,
		author_name,
		created: now - ago,
		modified: now - ago
	});
	state.versions.push(
		taskVersion(11, DAY, `${t.title} (draft)`, 'incomplete', 'created', 'agent', 'Core Engineer'),
		taskVersion(12, 40 * MINUTE, t.title, t.status, 'edited', 'user', null)
	);

	const lostTask = 9001;
	state.tasks.push({ ...t, id: lostTask, title: 'Renew the domain', deleted: true });
	state.versions.push(
		taskVersion(
			13,
			2 * DAY,
			'Renew the domain',
			'incomplete',
			'created',
			'user',
			null,
			'',
			lostTask
		),
		taskVersion(
			14,
			2 * 60 * MINUTE,
			'Renew the domain',
			'incomplete',
			'deleted',
			'agent',
			'Core Engineer',
			'',
			lostTask
		)
	);
	const lostDocument = 9002;
	const contents = '# Launch checklist\n\n- Write the post\n- Tell people\n';
	state.documents.push({
		...d,
		id: lostDocument,
		title: 'Launch checklist',
		contents,
		deleted: true
	});
	state.versions.push(
		{
			...version(6, 26 * 60 * MINUTE, contents, 'deleted', 'user', null),
			item: { kind: 'document', document_id: lostDocument },
			title: 'Launch checklist'
		},
		{
			...version(7, 4 * DAY, contents, 'created', 'user', null),
			item: { kind: 'document', document_id: lostDocument },
			title: 'Launch checklist'
		}
	);
	state.versions.sort((a, b) => a.created - b.created);
}

export function createMock(): Mock {
	const state = seed();
	seedRewind(state, nowSeconds());
	const nextId = (rows: { id: number }[]) =>
		rows.reduce((max, row) => Math.max(max, row.id), 0) + 1;
	const notFound = (what: string): never => {
		throw new ProtocolError('notFound', `The requested ${what} wasn’t found.`);
	};
	const notMocked = (what: string): never => {
		throw new ProtocolError('badRequest', `${what} aren’t in the mock yet.`);
	};
	const workspace = (id: number) =>
		state.workspaces.find((w) => w.id === id && !w.deleted) ?? notFound('workspace');
	const document = (workspace_id: number, id: number) =>
		state.documents.find((d) => d.id === id && d.workspace_id === workspace_id && !d.deleted) ??
		notFound('document');
	const liveDocumentFolders = (workspace_id: number) =>
		state.documentFolders
			.filter((f) => f.workspace_id === workspace_id && !f.deleted)
			.sort((a, b) => a.sort_order - b.sort_order || a.id - b.id);
	const documentFolder = (workspace_id: number, id: number) =>
		state.documentFolders.find(
			(f) => f.id === id && f.workspace_id === workspace_id && !f.deleted
		) ?? notFound('document folder');
	/** `root` and every live folder inside it. */
	const documentSubtree = (workspace_id: number, root: number): number[] => {
		const ids = [root];
		for (let i = 0; i < ids.length; i++)
			for (const f of liveDocumentFolders(workspace_id)) if (f.parent_id === ids[i]) ids.push(f.id);
		return ids;
	};
	// A contact as the daemon reads it: its row, with its methods by id.
	type MockContact = Omit<Contact, 'methods'>;
	const contactRow = (workspace_id: number, id: number) =>
		state.contacts.find((c) => c.id === id && c.workspace_id === workspace_id && !c.deleted) ??
		notFound('contact');
	const withMethods = (c: MockContact): Contact => ({
		...c,
		methods: state.contactMethods
			.filter((m) => m.contact_id === c.id)
			.sort((a, b) => a.id - b.id)
			.map((m) => ({ ...m, workspace_id: c.workspace_id }))
	});
	const contactMethod = (workspace_id: number, id: number) => {
		const m = state.contactMethods.find((m) => m.id === id) ?? notFound('contact method');
		contactRow(workspace_id, m.contact_id);
		return m;
	};
	// The names a contact needs, as the daemon checks them; an empty middle
	// name is none.
	const contactNames = (
		first_name: string,
		middle_name: string | null | undefined,
		last_name: string
	) => {
		if (first_name === '')
			throw new ProtocolError('validationFailed', 'First name can’t be empty.', 'first_name');
		if (last_name === '')
			throw new ProtocolError('validationFailed', 'Last name can’t be empty.', 'last_name');
		return { first_name, middle_name: middle_name || null, last_name };
	};
	const requireMethodValue = (value: string) => {
		if (value === '')
			throw new ProtocolError('validationFailed', 'A contact method can’t be empty.', 'value');
	};
	const touchContact = (id: number) => {
		const c = state.contacts.find((c) => c.id === id);
		if (c) c.modified = nowSeconds();
	};
	const addMethod = (contact_id: number, kind: ContactMethodKind, value: string) => {
		requireMethodValue(value);
		const now = nowSeconds();
		const created: MockContactMethod = {
			id: nextId(state.contactMethods),
			contact_id,
			kind,
			value,
			created_at: now,
			modified: now
		};
		state.contactMethods.push(created);
		touchContact(contact_id);
		return created;
	};
	const methodReply = (m: MockContactMethod): ContactMethod => ({
		...m,
		workspace_id: state.contacts.find((c) => c.id === m.contact_id)?.workspace_id ?? 0
	});
	const task = (workspace_id: number, id: number) =>
		state.tasks.find((t) => t.id === id && t.workspace_id === workspace_id && !t.deleted) ??
		notFound('task');
	const liveFolders = (workspace_id: number) =>
		state.folders
			.filter((f) => f.workspace_id === workspace_id && !f.deleted)
			.sort((a, b) => a.sort_order - b.sort_order || a.id - b.id);
	const folder = (workspace_id: number, id: number) =>
		state.folders.find((f) => f.id === id && f.workspace_id === workspace_id && !f.deleted) ??
		notFound('task folder');
	const savedViews = (workspace_id: number) =>
		state.savedTaskViews
			.filter((v) => v.workspace_id === workspace_id)
			.sort((a, b) => a.sort_order - b.sort_order || a.id - b.id);
	const savedView = (workspace_id: number, id: number) =>
		state.savedTaskViews.find((v) => v.id === id && v.workspace_id === workspace_id) ??
		notFound('saved task view');
	const ensureViewName = (workspace_id: number, name: string, id = 0) => {
		if (name === '') throw new ProtocolError('validationFailed', 'Name can’t be empty.', 'name');
		const lower = name.toLowerCase();
		if (savedViews(workspace_id).some((v) => v.id !== id && v.name.toLowerCase() === lower))
			throw new ProtocolError('validationFailed', `A view named “${name}” already exists.`, 'name');
	};
	/** The daemon refuses a folder or tag a view can't use. */
	const ensureViewFilters = (
		workspace_id: number,
		scope: SavedTaskView['scope'],
		tags: TagFilter
	) => {
		if (typeof scope === 'number') folder(workspace_id, scope);
		for (const id of [...tags.with, ...tags.without]) tagOf(workspace_id, id);
	};
	const feed = (id: number) =>
		state.feeds.find((f) => f.id === id && !f.deleted) ?? notFound('feed');
	const liveFeeds = () =>
		state.feeds
			.filter((f) => !f.deleted)
			.sort((a, b) => a.sort_order - b.sort_order || a.id - b.id);
	const feedSummary = (f: Feed): FeedSummary => ({
		...structuredClone(f),
		unread: state.articles.filter((l) => l.feed_id === f.id && !l.deleted && !l.read).length
	});
	/** Removes `feed_ids`' items, those published before `before` when given. */
	const removeItems = (feed_ids: number[], before?: number) => {
		let removed = 0;
		for (const l of state.articles)
			if (
				!l.deleted &&
				feed_ids.includes(l.feed_id) &&
				(before === undefined || l.published_at < before)
			) {
				l.deleted = true;
				l.contents = '';
				removed++;
			}
		return removed;
	};
	const summary = ({ contents: _contents, ...rest }: Article): ArticleSummary => rest;
	/** The ids of the workspace's tags on task `id`. */
	const tagIdsOf = (id: number): number[] =>
		state.tagged.filter((x) => x.task_id === id).map((x) => x.tag_id);
	const tagsOf = (workspace_id: number) =>
		state.tags
			.filter((t) => t.workspace_id === workspace_id)
			.sort((a, b) => a.sort_order - b.sort_order || a.id - b.id)
			.map((t) => ({ ...t }));
	const tagOf = (workspace_id: number, tag_id: number): Tag =>
		state.tags.find((t) => t.id === tag_id && t.workspace_id === workspace_id) ?? notFound('tag');
	const ensureTagName = (workspace_id: number, name: string, id = 0) => {
		if (name === '') throw new ProtocolError('validationFailed', 'Name can’t be empty.', 'name');
		const lower = name.toLowerCase();
		if (
			state.tags.some(
				(t) => t.id !== id && t.workspace_id === workspace_id && t.name.toLowerCase() === lower
			)
		)
			throw new ProtocolError('validationFailed', `A tag named “${name}” already exists.`, 'name');
	};
	/** Puts a task's tags back on its row, as the daemon reads them. */
	const withTags = (t: Task): Task => ({ ...t, tag_ids: tagIdsOf(t.id) });
	/** Whether `filter` covers an article. */
	const coveredBy =
		(filter: ArticleFilter) =>
		(l: Article): boolean =>
			filter === 'feeds' || l.feed_id === filter.feed;
	/**
	 * As the daemon's `excerpt`, roughly: the first line of body text, its marks
	 * dropped, passing over one that only repeats `title`.
	 */
	const words = (text: string) => (text.toLowerCase().match(/[\p{L}\p{N}]+/gu) ?? []).join(' ');
	const excerptOf = (text: string, title = ''): string => {
		const plain = text
			.split('\n')
			.map((l) => l.trim())
			.filter((l) => l !== '' && !/^(#|\||```|---)/.test(l))
			.map((l) =>
				l
					.replace(/^(>\s*)?([-*+]\s|\d+[.)]\s)?(\[[ xX]\]\s)?/, '')
					.replace(/!?\[([^\]]*)\]\([^)]*\)/g, '$1')
					.replace(/\*\*|__|~~|[*`]/g, '')
			)
			.find((l) => title === '' || words(l) !== words(title));
		return (plain ?? '').length > 200 ? `${plain!.slice(0, 199)}…` : (plain ?? '');
	};
	/**
	 * Whether Markdown `text` links to `target` anywhere, as an inline link's
	 * target or an autolink outside fenced code, as the daemon's
	 * `outgoing_links` finds them.
	 */
	const linksTo = (text: string, target: { kind: string; id: number }): boolean => {
		let fenced = false;
		for (const line of text.split('\n')) {
			if (/^\s*(```|~~~)/.test(line)) {
				fenced = !fenced;
				continue;
			}
			if (fenced) continue;
			for (const [, href] of line.matchAll(/(?:\]\(|<)(cohesion:\/\/[^\s)>"']+)/g)) {
				const link = parseItemLink(href);
				if (link?.kind === target.kind && link.id === target.id) return true;
			}
		}
		return false;
	};
	/** The live item a `cohesion://` link names, as the daemon's `ResolveLinkCommand` finds it. */
	const resolveLink = (link: string): Embed => {
		const match =
			/^cohesion:\/\/(task|document|resource|agent|actor-session|article)\/(\d+)(?:\/(\d+))?$/.exec(
				link.trim()
			);
		const scoped = match?.[3] !== undefined;
		const kind = match?.[1];
		if (
			!match ||
			(scoped ? !['task', 'document', 'resource'].includes(kind!) : kind === 'resource')
		)
			throw new ProtocolError(
				'validationFailed',
				'That isn’t a link to a Cohesion item, such as cohesion://task/3/42.',
				'link'
			);
		const id = Number(scoped ? match[3] : match[2]);
		const ws = scoped ? Number(match[2]) : null;
		switch (kind) {
			case 'task': {
				const found =
					state.tasks.find(
						(t) => t.id === id && !t.deleted && (ws === null || t.workspace_id === ws)
					) ?? notFound('task');
				workspace(found.workspace_id);
				return {
					link: `cohesion://task/${found.workspace_id}/${id}`,
					kind: 'task',
					workspace_id: found.workspace_id,
					title: found.title,
					excerpt: excerptOf(found.description, found.title),
					item: { kind: 'task', task: withTags(found) }
				};
			}
			case 'document': {
				const found =
					state.documents.find(
						(d) => d.id === id && !d.deleted && (ws === null || d.workspace_id === ws)
					) ?? notFound('document');
				workspace(found.workspace_id);
				return {
					link: `cohesion://document/${found.workspace_id}/${id}`,
					kind: found.document_type === 'journal' ? 'journalEntry' : 'document',
					workspace_id: found.workspace_id,
					title: found.title,
					excerpt: excerptOf(found.contents, found.title),
					item: { kind: 'document', document: found }
				};
			}
			case 'resource': {
				const found = resource(workspace(ws!).id, id);
				return {
					link: `cohesion://resource/${ws}/${id}`,
					kind: 'resource',
					workspace_id: ws,
					title: found.title,
					excerpt: found.content_kind === 'json' ? '' : excerptOf(found.contents, found.title),
					item: { kind: 'resource', resource: found }
				};
			}
			case 'agent': {
				const found = agentRow(agent(id));
				return {
					link: `cohesion://agent/${id}`,
					kind: 'agent',
					workspace_id: null,
					title: found.agent_name,
					excerpt: excerptOf(found.agent_role),
					item: { kind: 'agent', agent: found }
				};
			}
			case 'actor-session': {
				const session = actorSession(id);
				const actor = state.actors.find((a) => a.id === session.actor_id && !a.deleted);
				const latest = state.actorSessionMessages.findLast(
					(m) => m.session_id === id && m.content.sender === 'actor' && m.content.kind === 'text'
				)?.content;
				return {
					link: `cohesion://actor-session/${id}`,
					kind: 'actorSession',
					workspace_id: null,
					title: session.title,
					excerpt: latest && 'text' in latest ? excerptOf(latest.text) : '',
					item: { kind: 'actorSession', session, actor_title: actor?.title ?? null }
				};
			}
			default: {
				const found = article(id);
				return {
					link: `cohesion://article/${id}`,
					kind: 'article',
					workspace_id: null,
					title: found.title,
					excerpt: excerptOf(found.contents, found.title),
					item: { kind: 'article', article: summary(found), feed_title: feed(found.feed_id).title }
				};
			}
		}
	};
	const shownKinds = (workspace_id: number) =>
		state.shownKinds
			.filter((k) => k.workspace_id === workspace_id)
			.sort((a, b) => a.sort_order - b.sort_order || a.id - b.id)
			.map((k) => ({ ...k }));
	/** Extensions granted `registry`, by priority, then oldest first. */
	const grantedRegistries = () =>
		state.extensions
			.filter((e) =>
				state.permissions.some(
					(p) => p.extension_id === e.id && p.permission === 'registry' && p.enabled
				)
			)
			.sort((a, b) => a.priority - b.priority || a.id - b.id);
	/** Each installed extension a registry lists at a higher version; the first registry wins. */
	const extensionUpdates = (): ExtensionUpdate[] => {
		const updates: ExtensionUpdate[] = [];
		for (const registry of grantedRegistries()) {
			const results = state.registries.find((r) => r.extension_id === registry.id)?.results ?? [];
			for (const result of results) {
				const installed = state.extensions.find(
					(e) => e.script_identifier === result.manifest.identifier
				);
				if (
					!installed ||
					result.manifest.version <= installed.version ||
					updates.some((u) => u.extension_id === installed.id)
				)
					continue;
				updates.push({
					extension_id: installed.id,
					registry_extension_id: registry.id,
					result: structuredClone(result)
				});
			}
		}
		return updates;
	};
	const journalFor = (workspace_id: number, date: string): Document => {
		workspace(workspace_id);
		const existing = state.documents.find(
			(d) => d.workspace_id === workspace_id && d.journal_date === date && !d.deleted
		);
		if (existing) return existing;
		const created: Document = {
			id: nextId(state.documents),
			workspace_id,
			document_type: 'journal',
			title: date,
			contents: '',
			modified: nowSeconds(),
			journal_date: date,
			deleted: false,
			folder_id: null
		};
		state.documents.push(created);
		return created;
	};
	// Rewind, as the daemon's triggers keep it: an edit within two minutes of
	// the latest version's start joins it, and anything else is a new one. Every
	// write the mock makes is by the user.
	const joins = (latest: { change: VersionChange; created: number } | undefined) =>
		latest !== undefined &&
		['created', 'edited', 'restored'].includes(latest.change) &&
		latest.created > nowSeconds() - 120;
	/** The item a version is of, as `kind:id`. */
	const itemKey = (v: Version) =>
		v.item.kind === 'task' ? `task:${v.item.task_id}` : `document:${v.item.document_id}`;
	const keep = (next: Version, change: VersionChange) => {
		const latest = state.versions.findLast((v) => itemKey(v) === itemKey(next));
		if (change === 'edited' && latest && joins(latest)) {
			Object.assign(latest, {
				item: next.item,
				title: next.title,
				body: next.body,
				modified: next.modified,
				author: 'user',
				author_name: null,
				change: latest.change === 'restored' ? 'edited' : latest.change
			});
			return;
		}
		state.versions.push({ ...next, id: nextId(state.versions), change });
	};
	const keepDocument = (d: Document, change: VersionChange) =>
		keep(
			{
				id: 0,
				item: { kind: 'document', document_id: d.id },
				workspace_id: d.workspace_id,
				title: d.title,
				body: d.contents,
				change,
				author: 'user',
				author_name: null,
				created: nowSeconds(),
				modified: nowSeconds()
			},
			change
		);
	const keepTask = (t: Task, change: VersionChange) =>
		keep(
			{
				id: 0,
				item: { kind: 'task', task_id: t.id, status: t.status },
				workspace_id: t.workspace_id,
				title: t.title,
				body: t.description,
				change,
				author: 'user',
				author_name: null,
				created: nowSeconds(),
				modified: nowSeconds()
			},
			change
		);
	const versionedDeleted = (v: Version) =>
		v.item.kind === 'task'
			? state.tasks.find((t) => t.id === (v.item as { task_id: number }).task_id)?.deleted
			: state.documents.find((d) => d.id === (v.item as { document_id: number }).document_id)
					?.deleted;
	const versionSummary = (v: Version): VersionSummary => ({
		id: v.id,
		item: v.item,
		workspace_id: v.workspace_id,
		change: v.change,
		author: v.author,
		author_name: v.author_name,
		modified: v.modified,
		current:
			!versionedDeleted(v) && state.versions.findLast((o) => itemKey(o) === itemKey(v))?.id === v.id
	});
	const notFoundVersion = () =>
		new ProtocolError('notFound', 'The requested version wasn’t found.');
	const touch = (d: Document, contents: string): Document => {
		d.contents = contents;
		d.modified = nowSeconds();
		keepDocument(d, 'edited');
		return { ...d };
	};
	// Workspaces and documents need a title, as the daemon checks it: not
	// trimmed, and without naming a field. Tasks and folders take any title.
	const requireTitle = (title: string) => {
		if (title === '') throw new ProtocolError('validationFailed', 'Title can’t be empty.');
	};
	// Trimmed, blank meaning none, and at most a few characters, as the daemon stores it.
	const emojiValue = (emoji: string | null | undefined) => {
		const trimmed = emoji?.trim() ?? '';
		if ([...trimmed].length > 8)
			throw new ProtocolError('validationFailed', 'Use a single emoji.', 'emoji');
		return trimmed === '' ? null : trimmed;
	};
	const agent = (id: number) =>
		state.agents.find((a) => a.id === id) ?? notFound('intelligence agent');
	// The daemon counts these when it reads an agent; so does the mock.
	const agentRow = (a: IntelligenceAgent): IntelligenceAgent => ({
		...a,
		following_count: state.actorSessions.filter(
			(t) => t.follower?.agent_id === a.id && t.status !== 'resolved'
		).length
	});
	const agentTeam = (id: number) =>
		state.agentTeams.find((t) => t.id === id) ?? notFound('intelligence agent team');
	// Trimmed, not blank, and unique ignoring case, as the team table holds it.
	const teamTitle = (title: string, except?: number) => {
		const trimmed = title.trim();
		requireTitle(trimmed);
		if (
			state.agentTeams.some(
				(t) => t.id !== except && t.title.toLowerCase() === trimmed.toLowerCase()
			)
		)
			throw new ProtocolError(
				'validationFailed',
				`There's already a team named “${trimmed}”.`,
				'title'
			);
		return trimmed;
	};
	const sortedTeams = () =>
		state.agentTeams.toSorted((a, b) => a.title.localeCompare(b.title)).map((t) => ({ ...t }));
	/**
	 * Pinned first in their order, then the rest by latest message (or creation), newest first.
	 * The daemon's trigger keeps `last_message_at`; the mock reads it off the messages.
	 */
	const agentActivity = (a: IntelligenceAgent) =>
		Math.max(
			a.created_at,
			...state.agentMessages
				.filter(
					(m) =>
						m.agent_id === a.id &&
						![
							'codeExecution',
							'documentation',
							'memory',
							'askedAgent',
							'askedByAgent',
							'note',
							'woke',
							'rebuiltContext'
						].includes(m.message_content.kind)
				)
				.map((m) => m.sent_at)
		);
	const sortedAgents = () =>
		state.agents.toSorted((a, b) =>
			a.pinned !== b.pinned
				? Number(b.pinned) - Number(a.pinned)
				: a.pinned
					? a.sort_order - b.sort_order || b.id - a.id
					: agentActivity(b) - agentActivity(a) || b.id - a.id
		);
	const agentMessageById = (id: number) =>
		state.agentMessages.find((m) => m.id === id) ?? notFound('agent message');
	/** What previewing `source` shows: its script and manifest, and the extension it would replace. */
	const preview = (source: MockSource) => {
		const installed = state.extensions.find(
			(e) => e.script_identifier === source.manifest.identifier
		);
		return {
			script: source.script,
			manifest: structuredClone(source.manifest),
			installed: installed ? { ...installed } : null
		};
	};
	/** Whether `extension_id` has `permission` enabled. */
	const granted = (extension_id: number, permission: ExtensionPermissionKind) =>
		state.permissions.some(
			(p) => p.extension_id === extension_id && p.permission === permission && p.enabled
		);
	/**
	 * An extension's connector, refusing it as the daemon does unless each of
	 * `permissions` is enabled: `oauth` to sign in, `connector` and `oauth` to
	 * import.
	 */
	const connector = (
		extension_id: number,
		permissions: ExtensionPermissionKind[] = ['connector', 'oauth']
	): MockConnector => {
		const missing = permissions.find((p) => !granted(extension_id, p));
		if (missing)
			throw new ProtocolError(
				'validationFailed',
				`This extension doesn't have the \`${missing}\` permission enabled.`
			);
		const found = state.connectors.find((c) => c.extension_id === extension_id);
		if (!found) throw new ProtocolError('upstreamError', 'This mock extension serves no service.');
		if (found.error) throw new ProtocolError('upstreamError', found.error);
		return found;
	};
	const connectorAccount = (id: number) =>
		state.connectorAccounts.find((a) => a.id === id) ?? notFound('connector account');
	/** The stored definition `id`. */
	const definition = (id: number) => state.definitions.find((d) => d.id === id)!;
	/**
	 * An account as it is sent: its connector, and whether it can import: an
	 * extension's when it is granted `connector`, an orphan's never.
	 */
	const redacted = ({
		host,
		connector_id,
		...account
	}: MockConnectorAccount): RedactedConnectorAccount => ({
		...account,
		host,
		connector: structuredClone(definition(connector_id)),
		importable: host.kind === 'extension' && granted(host.extension_id, 'connector')
	});
	/** Refuses an orphaned account, which nothing serves, as the daemon does. */
	const served = (account: MockConnectorAccount) => {
		if (account.host.kind === 'orphaned')
			throw new ProtocolError(
				'validationFailed',
				`${account.description} is no longer connected; move its resources to another account.`
			);
	};
	/** Whether client `client_id` is connected. */
	const clientServes = (client_id: number) => state.onlineClients.includes(client_id);
	const actorSession = (id: number) =>
		state.actorSessions.find((t) => t.id === id) ?? notFound('actor session');
	/** The sessions below session `id`, at any depth. */
	const subagentsBelow = (id: number): ActorSession[] =>
		state.actorSessions
			.filter((t) => t.subagent?.parent_session_id === id)
			.flatMap((t) => [t, ...subagentsBelow(t.id)]);
	/** Session `id`, which must be one Cohesion started: a subagent goes with its session. */
	const startedSession = (id: number) => {
		const session = actorSession(id);
		if (session.subagent)
			throw new ProtocolError('badRequest', 'A subagent goes with its session.');
		return session;
	};
	/** Keeps the images sent with actor session message `message_id`. */
	const attachActorImages = (message_id: number, images: ImageUpload[] | null | undefined) => {
		for (const { media_type, data } of images ?? [])
			state.actorSessionAttachments.push({
				id: nextId(state.actorSessionAttachments),
				message: { kind: 'actorSession', actor_session_message_id: message_id },
				media_type,
				data
			});
	};
	/**
	 * Fetches a page as the daemon would: hosts under `.example` can't be
	 * reached, and any other address reads as a short page titled after
	 * its path.
	 */
	const fetchPage = (url: URL): { capture: MockCapture } | { failure: ArticleFetchFailure } => {
		if (url.hostname.endsWith('.example'))
			return { failure: { kind: 'unreachable', message: 'dns error: failed to lookup address' } };
		const name = url.pathname.split('/').filter(Boolean).pop() ?? url.hostname;
		return {
			capture: {
				final_url: url.href,
				page_title: name,
				text: `${name}\n\nA page from ${url.hostname}.`,
				truncated: false
			}
		};
	};
	/** Stores a fetch on `found` the way `ResnapshotArticleCommand` does. */
	const record = (found: Article, url: URL): RefetchedArticle => {
		const fetched = fetchPage(url);
		const now = nowSeconds();
		const neverCaptured = found.snapshot.state === 'failed';
		if ('capture' in fetched) {
			const { capture } = fetched;
			if (neverCaptured && found.title === fallbackTitle(url) && capture.page_title)
				found.title = capture.page_title;
			found.snapshot = {
				state: 'captured',
				final_url: capture.final_url,
				page_title: capture.page_title,
				fetched_at: now,
				truncated: capture.truncated
			};
			found.contents = capture.text;
			found.modified = now;
			return { article: structuredClone(found), failure: null };
		}
		if (neverCaptured) {
			found.snapshot = { state: 'failed', failure: fetched.failure, attempted_at: now };
			found.modified = now;
		}
		return { article: structuredClone(found), failure: fetched.failure };
	};
	const article = (id: number) =>
		state.articles.find((l) => l.id === id && !l.deleted) ?? notFound('article');
	const resource = (workspace_id: number, id: number) =>
		state.resources.find((a) => a.id === id && a.workspace_id === workspace_id) ??
		notFound('resource');
	/**
	 * The record `identifier` on the service behind `account`, with its kind
	 * as a read composes it.
	 */
	const remoteResource = (account: MockConnectorAccount, identifier: string) => {
		served(account);
		const { host } = account;
		const prefix = definition(account.connector_id).identifier;
		if (host.kind !== 'extension') throw new Error('served() refuses an orphan');
		const found = connector(host.extension_id).resources.find((a) => a.identifier === identifier);
		if (!found)
			throw new ProtocolError(
				'upstreamError',
				`${definition(account.connector_id).title} has no ${identifier}.`
			);
		return { ...found, kind: `${prefix}.${found.kind_identifier}` };
	};
	/** Orphans the accounts `match` picks, as revoking or uninstalling their host does. */
	const orphan = (match: (account: MockConnectorAccount) => boolean) => {
		for (const account of state.connectorAccounts.filter(match)) {
			account.description = redacted(account).description;
			account.host = { kind: 'orphaned' };
		}
	};
	/** Endpoint `endpoint_id`'s models, ordered as `ListEndpointModelsCommand` orders them. */
	const modelsOf = (endpoint_id: number) =>
		state.endpointModels
			.filter((m) => m.endpoint_id === endpoint_id)
			// Code-unit order, as SQLite's binary collation sorts.
			.sort((a, b) =>
				a.model_name !== b.model_name
					? a.model_name < b.model_name
						? -1
						: 1
					: a.model_identifier < b.model_identifier
						? -1
						: 1
			);
	/** Drops endpoint models and, as the cascade does, the tiers that use them. */
	const dropModels = (drop: (model: EndpointModel) => boolean) => {
		const gone = new Set(state.endpointModels.filter(drop).map((m) => m.id));
		state.endpointModels = state.endpointModels.filter((m) => !gone.has(m.id));
		state.defaults = state.defaults.filter((d) => !gone.has(d.endpoint_model_id));
	};
	/** Adds or updates a model the way `AddEndpointModelCommand` does. */
	const addEndpointModel = ({
		endpoint_id,
		model_identifier,
		model_name,
		context_size,
		prices
	}: Requests['addEndpointModel']): EndpointModel => {
		const { backend } =
			state.endpoints.find((e) => e.id === endpoint_id) ?? notFound('intelligence endpoint');
		const existing = state.endpointModels.find(
			(m) => m.endpoint_id === endpoint_id && m.model_identifier === model_identifier
		);
		const id = existing?.id ?? nextId(state.endpointModels);
		const catalog = state.catalog.find(
			(m) =>
				m.provider_identifier === backend.provider_identifier &&
				m.model_identifier === model_identifier
		);
		// The request is the whole model; what it leaves out falls back to the catalog.
		const base = catalog ? fromCatalog(id, endpoint_id, catalog) : null;
		const model: EndpointModel = {
			id,
			endpoint_id,
			model_identifier,
			model_name: model_name ?? base?.model_name ?? model_identifier,
			catalog_model_id: catalog?.id ?? null,
			context_size: context_size ?? base?.context_size ?? null,
			input_token_cost: prices?.input_token_cost ?? base?.input_token_cost ?? null,
			output_token_cost: prices?.output_token_cost ?? base?.output_token_cost ?? null,
			cache_read_token_cost: prices?.cache_read_token_cost ?? base?.cache_read_token_cost ?? null,
			cache_write_token_cost:
				prices?.cache_write_token_cost ?? base?.cache_write_token_cost ?? null,
			large_context_threshold: base?.large_context_threshold ?? null,
			large_context_input_token_cost: base?.large_context_input_token_cost ?? null,
			large_context_output_token_cost: base?.large_context_output_token_cost ?? null,
			large_context_cache_read_token_cost: base?.large_context_cache_read_token_cost ?? null,
			large_context_cache_write_token_cost: base?.large_context_cache_write_token_cost ?? null
		};
		state.endpointModels = existing
			? state.endpointModels.map((m) => (m.id === id ? model : m))
			: [...state.endpointModels, model];
		return { ...model };
	};

	/** Records a security event, as the daemon does. */
	const recordSecurity = (
		kind: SecurityEventKind,
		client: Client | null,
		address: string | null = null
	): void => {
		state.securityEvents.push(
			securityEvent(
				nextId(state.securityEvents),
				nowSeconds(),
				kind,
				client?.id ?? null,
				client?.client_name ?? null,
				address
			)
		);
	};
	/** Why the page, as its current client, can't open pairing, as the daemon would say. */
	const pairingRefusal = (): string | null => {
		const asking = state.clients.find((c) => c.id === state.currentClient);
		return state.security.pairing_opened_by === 'localClients' && asking?.key_kind !== 'local'
			? 'Clients can only be added from a client on the computer Cohesion runs on. Change this under Add clients from in Settings › Security.'
			: null;
	};
	/** Revokes client `id`, as `deleteClient` does, and records it. */
	const revokeClient = (id: number): Client => {
		const client = state.clients.find((c) => c.id === id && !c.deleted) ?? notFound('client');
		client.deleted = true;
		state.onlineClients = state.onlineClients.filter((c) => c !== id);
		recordSecurity('clientRevoked', client);
		return { ...client };
	};

	const handlers: Handlers = {
		about: () => ({ hostname: 'mock.local', version: '0.0.0-mock' }),
		// The mock is the page itself, so like a daemon built without web/build it has no bundle.
		webBundle: () => {
			throw new ProtocolError(
				'badRequest',
				'This cohesiond was built without the web UI; run npm run build in web/ and rebuild it.'
			);
		},
		webBundleHash: () => ({ hash: null }),
		listWorkspaces: () => state.workspaces.filter((w) => !w.deleted).map((w) => ({ ...w })),
		listIntelligenceDefaults: () => state.defaults.map((d) => ({ ...d })),
		listClients: () =>
			state.clients
				.filter((c) => !c.deleted)
				.sort((a, b) =>
					a.client_name.localeCompare(b.client_name, undefined, { sensitivity: 'base' })
				)
				.map((c) => ({
					...c,
					online: state.onlineClients.includes(c.id),
					current: c.id === state.currentClient
				})),
		beginPairing: () => {
			const refusal = pairingRefusal();
			if (refusal) throw new ProtocolError('badRequest', refusal);
			const asking = state.clients.find((c) => c.id === state.currentClient);
			recordSecurity('pairingOpened', asking ?? null, asking?.last_address ?? null);
			return { code: btoa('mock-key|127.0.0.1|mock-fingerprint') };
		},
		getPairingAccess: () => ({ refusal: pairingRefusal() }),
		revokeOtherClients: () => {
			const others = state.clients.filter((c) => !c.deleted && c.id !== state.currentClient);
			return others.map((c) => revokeClient(c.id));
		},
		getSecuritySettings: () => ({ ...state.security }),
		updateSecuritySettings: (settings) => {
			state.security = { id: 1, ...settings };
			recordSecurity('settingsChanged', null);
			return { ...state.security };
		},
		getStorageSettings: () => ({ ...state.storage }),
		updateStorageSettings: (settings) => {
			if (Object.values(settings).some((value) => !(value >= 1)))
				throw new ProtocolError('validationFailed', 'A value is out of range.');
			state.storage = { id: 1, ...settings };
			return { ...state.storage };
		},
		getDatabaseSize: () => ({ ...state.databaseSize }),
		backupDatabase: () => ({
			path: '/home/you/.config/cohesiond/backups/cohesion-2026-11-03-120000.db',
			bytes: state.databaseSize.bytes
		}),
		// Moving data between daemons is the CLI's (`cohesion export` and
		// `cohesion import`); the web UI never sends these.
		exportDatabase: noTransfers,
		readDatabaseExport: noTransfers,
		beginDatabaseImport: noTransfers,
		writeDatabaseImport: noTransfers,
		finishDatabaseImport: noTransfers,
		getProfile: () => ({ ...state.profile }),
		setProfileName: ({ name }) => {
			state.profile.name = name?.trim() || null;
			return { ...state.profile };
		},
		setProfileBio: ({ bio }) => {
			const trimmed = bio?.trim() || null;
			if (trimmed && [...trimmed].length > 1000)
				throw new ProtocolError('validationFailed', 'Keep your bio to 1,000 characters.', 'bio');
			state.profile.bio = trimmed;
			return { ...state.profile };
		},
		setProfilePhoto: ({ photo }) => {
			state.profile.photo = photo
				? { kind: 'image', photo_media_type: photo.media_type, photo_data: photo.data }
				: { kind: 'none' };
			return { ...state.profile };
		},
		listSecurityEvents: ({ offset = 0, limit } = {}) => {
			const newest = [...state.securityEvents].reverse();
			return {
				total: newest.length,
				events: newest.slice(offset, offset + (limit ?? 50)).map((e) => ({ ...e }))
			};
		},
		listExtensions: () => state.extensions.map((e) => ({ ...e })),

		renameClient: ({ id, client_name }) => {
			const client = state.clients.find((c) => c.id === id && !c.deleted) ?? notFound('client');
			client.client_name = client_name.trim();
			recordSecurity('clientRenamed', client);
			return { ...client };
		},

		deleteClient: ({ id }) => revokeClient(id),

		createWorkspace: ({ title, emoji }) => {
			requireTitle(title);
			const created = {
				id: nextId(state.workspaces),
				title,
				emoji: emojiValue(emoji),
				deleted: false
			};
			state.workspaces.push(created);
			return { ...created };
		},
		renameWorkspace: ({ id, title }) => {
			const w = workspace(id);
			w.title = title;
			return { ...w };
		},
		setWorkspaceEmoji: ({ id, emoji }) => {
			const w = workspace(id);
			w.emoji = emojiValue(emoji);
			return { ...w };
		},
		deleteWorkspace: ({ id }) => {
			const w = workspace(id);
			w.deleted = true;
			return { ...w };
		},

		getJournal: ({ workspace_id, date }) => {
			workspace(workspace_id);
			const existing = state.documents.find(
				(d) => d.workspace_id === workspace_id && d.journal_date === date && !d.deleted
			);
			return existing ? { ...existing } : null;
		},
		editJournal: ({ workspace_id, date, content }) =>
			touch(journalFor(workspace_id, date), content),
		appendToJournal: ({ workspace_id, date, content }) => {
			const d = journalFor(workspace_id, date);
			const joined = d.contents === '' ? content : `${d.contents.replace(/\n*$/, '')}\n${content}`;
			return touch(d, joined);
		},
		searchDocuments: ({ workspace_id, query }) => {
			const q = query.toLowerCase();
			return state.documents
				.filter(
					(d) => d.workspace_id === workspace_id && !d.deleted && d.title.toLowerCase().includes(q)
				)
				.map((d) => ({ ...d }));
		},
		listDocuments: ({ workspace_id, document_type, journal_from, journal_to }) =>
			state.documents
				.filter(
					(d) =>
						d.workspace_id === workspace_id &&
						!d.deleted &&
						(document_type == null || d.document_type === document_type) &&
						(journal_from == null || (d.journal_date !== null && d.journal_date >= journal_from)) &&
						(journal_to == null || (d.journal_date !== null && d.journal_date < journal_to))
				)
				.map((d) => ({ ...d })),
		getDocument: ({ workspace_id, document_id }) => ({ ...document(workspace_id, document_id) }),
		editDocument: ({ workspace_id, document_id, contents }) =>
			touch(document(workspace_id, document_id), contents),
		createDocument: ({ workspace_id, title, contents, folder_id = null }) => {
			workspace(workspace_id);
			requireTitle(title);
			if (folder_id !== null) documentFolder(workspace_id, folder_id);
			const created: Document = {
				id: nextId(state.documents),
				workspace_id,
				document_type: 'normal',
				title,
				contents,
				modified: nowSeconds(),
				journal_date: null,
				deleted: false,
				folder_id
			};
			state.documents.push(created);
			keepDocument(created, 'created');
			return { ...created };
		},
		moveDocument: ({ workspace_id, document_id, folder_id = null }) => {
			const d = document(workspace_id, document_id);
			if (d.document_type === 'journal')
				throw new ProtocolError('validationFailed', 'A journal can’t go in a folder.');
			if (folder_id !== null) documentFolder(workspace_id, folder_id);
			d.folder_id = folder_id;
			return { ...d };
		},
		listDocumentFolders: ({ workspace_id }) => {
			workspace(workspace_id);
			return liveDocumentFolders(workspace_id).map((f) => ({ ...f }));
		},
		createDocumentFolder: ({ workspace_id, title, parent_id = null }) => {
			workspace(workspace_id);
			requireTitle(title);
			if (parent_id !== null) documentFolder(workspace_id, parent_id);
			const created: DocumentFolder = {
				id: nextId(state.documentFolders),
				workspace_id,
				parent_id,
				title,
				sort_order:
					Math.max(
						0,
						...state.documentFolders
							.filter((f) => f.workspace_id === workspace_id)
							.map((f) => f.sort_order)
					) + 1,
				deleted: false
			};
			state.documentFolders.push(created);
			return { ...created };
		},
		renameDocumentFolder: ({ workspace_id, folder_id, title }) => {
			requireTitle(title);
			const f = documentFolder(workspace_id, folder_id);
			f.title = title;
			return { ...f };
		},
		moveDocumentFolder: ({ workspace_id, folder_id, parent_id = null, before_id = null }) => {
			const f = documentFolder(workspace_id, folder_id);
			if (parent_id !== null) {
				documentFolder(workspace_id, parent_id);
				if (documentSubtree(workspace_id, folder_id).includes(parent_id))
					throw new ProtocolError(
						'validationFailed',
						'A folder can’t move into itself or a folder inside it.'
					);
			}
			const siblings = liveDocumentFolders(workspace_id).filter(
				(s) => s.parent_id === parent_id && s.id !== folder_id
			);
			const at =
				before_id === null ? siblings.length : siblings.findIndex((s) => s.id === before_id);
			if (at < 0)
				throw new ProtocolError(
					'validationFailed',
					'The folder to go before isn’t in that folder.'
				);
			siblings.splice(at, 0, f);
			f.parent_id = parent_id;
			siblings.forEach((s, i) => (s.sort_order = i));
			return liveDocumentFolders(workspace_id).map((s) => ({ ...s }));
		},
		deleteDocumentFolder: ({ workspace_id, folder_id }) => {
			const f = documentFolder(workspace_id, folder_id);
			const doomed = documentSubtree(workspace_id, folder_id);
			// The daemon moves their documents, deleted ones too, to the Inbox.
			for (const d of state.documents)
				if (d.workspace_id === workspace_id && d.folder_id !== null && doomed.includes(d.folder_id))
					d.folder_id = null;
			for (const id of doomed) documentFolder(workspace_id, id).deleted = true;
			return { ...f };
		},
		// Databases aren't in the mock yet: it has none, and refuses to make one.
		listDatabases: ({ workspace_id }) => {
			workspace(workspace_id);
			return [];
		},
		getDatabase: () => notFound('database'),
		createDatabase: () => notMocked('Databases'),
		renameDatabase: () => notFound('database'),
		deleteDatabase: () => notFound('database'),
		listDatabaseProperties: () => notFound('database'),
		createDatabaseProperty: () => notFound('database'),
		renameDatabaseProperty: () => notFound('database'),
		setDatabasePropertyType: () => notFound('database'),
		setDatabasePropertyFormula: () => notFound('database'),
		moveDatabaseProperty: () => notFound('database'),
		deleteDatabaseProperty: () => notFound('database'),
		listDatabaseRows: () => notFound('database'),
		getDatabaseRow: () => notFound('database'),
		createDatabaseRow: () => notFound('database'),
		deleteDatabaseRow: () => notFound('database'),
		setDatabaseValue: () => notFound('database'),
		createDatabaseRowDocument: () => notFound('database'),
		renameDocument: ({ workspace_id, document_id, title }) => {
			requireTitle(title);
			const d = document(workspace_id, document_id);
			d.title = title;
			keepDocument(d, 'edited');
			return { ...d };
		},
		deleteDocument: ({ workspace_id, document_id }) => {
			const d = document(workspace_id, document_id);
			d.deleted = true;
			keepDocument(d, 'deleted');
			return { ...d };
		},

		listVersions: ({ workspace_id, item }) =>
			state.versions
				.filter(
					(v) =>
						v.workspace_id === workspace_id &&
						(item.kind === 'task'
							? v.item.kind === 'task' && v.item.task_id === item.task_id
							: v.item.kind === 'document' && v.item.document_id === item.document_id)
				)
				.map(versionSummary)
				.reverse(),
		getVersion: ({ workspace_id, version_id }) => {
			const v = state.versions.find((v) => v.workspace_id === workspace_id && v.id === version_id);
			if (!v) throw notFoundVersion();
			return { ...v };
		},
		restoreVersion: ({ workspace_id, version_id }) => {
			const v = state.versions.find((v) => v.workspace_id === workspace_id && v.id === version_id);
			if (!v) throw notFoundVersion();
			if (v.item.kind === 'task') {
				const { task_id, status } = v.item;
				const t = state.tasks.find((t) => t.id === task_id);
				if (!t) throw notFoundVersion();
				const unchanged =
					t.title === v.title && t.description === v.body && t.status === status && !t.deleted;
				if (unchanged) return { kind: 'task', task: { ...t } };
				const folder = state.folders.find((f) => f.id === t.task_folder_id);
				if (folder?.deleted) t.task_folder_id = null;
				Object.assign(t, { title: v.title, description: v.body, status, deleted: false });
				keepTask(t, 'restored');
				return { kind: 'task', task: { ...t } };
			}
			const { document_id } = v.item;
			const d = state.documents.find((d) => d.id === document_id);
			if (!d) throw notFoundVersion();
			if (d.title === v.title && d.contents === v.body && !d.deleted)
				return { kind: 'document', document: { ...d } };
			Object.assign(d, {
				title: v.title,
				contents: v.body,
				deleted: false,
				modified: nowSeconds()
			});
			keepDocument(d, 'restored');
			return { kind: 'document', document: { ...d } };
		},
		listDeletedItems: ({ workspace_id }) => {
			const latestDeleted = (key: string) =>
				state.versions.findLast((v) => itemKey(v) === key && v.change === 'deleted');
			const row = (
				v: Version | undefined,
				item: DeletedItem['item'],
				journal_date: string | null
			): DeletedItem[] =>
				v
					? [
							{
								item,
								title: v.title,
								journal_date,
								author: v.author,
								author_name: v.author_name,
								modified: v.modified
							}
						]
					: [];
			const tasks = state.tasks
				.filter((t) => t.workspace_id === workspace_id && t.deleted)
				.flatMap((t) => {
					const v = latestDeleted(`task:${t.id}`);
					return row(v, { kind: 'task', task_id: t.id, status: t.status }, null);
				});
			const documents = state.documents
				.filter((d) => d.workspace_id === workspace_id && d.deleted)
				.flatMap((d) =>
					row(
						latestDeleted(`document:${d.id}`),
						{ kind: 'document', document_id: d.id },
						d.journal_date
					)
				);
			return [...tasks, ...documents].sort((a, b) => b.modified - a.modified);
		},

		listContacts: ({ workspace_id }) => {
			workspace(workspace_id);
			const name = (c: MockContact) => [c.last_name.toLowerCase(), c.first_name.toLowerCase()];
			return state.contacts
				.filter((c) => c.workspace_id === workspace_id && !c.deleted)
				.sort((a, b) => {
					const [al, af] = name(a);
					const [bl, bf] = name(b);
					return al.localeCompare(bl) || af.localeCompare(bf) || a.id - b.id;
				})
				.map(withMethods);
		},
		getContact: ({ workspace_id, contact_id }) => withMethods(contactRow(workspace_id, contact_id)),
		createContact: ({ workspace_id, first_name, middle_name, last_name, notes, methods }) => {
			workspace(workspace_id);
			const names = contactNames(first_name, middle_name, last_name);
			methods.forEach((m) => requireMethodValue(m.value));
			const now = nowSeconds();
			const created: MockContact = {
				id: nextId(state.contacts),
				workspace_id,
				deleted: false,
				...names,
				notes,
				created_at: now,
				modified: now,
				photo: { kind: 'none' }
			};
			state.contacts.push(created);
			methods.forEach((m) => addMethod(created.id, m.kind, m.value));
			return withMethods(created);
		},
		updateContact: ({ workspace_id, contact_id, first_name, middle_name, last_name, notes }) => {
			const names = contactNames(first_name, middle_name, last_name);
			const c = contactRow(workspace_id, contact_id);
			Object.assign(c, names, { notes, modified: nowSeconds() });
			return withMethods(c);
		},
		deleteContact: ({ workspace_id, contact_id }) => {
			const c = contactRow(workspace_id, contact_id);
			c.deleted = true;
			for (const t of state.tasks)
				if (t.assignee?.kind === 'contact' && t.assignee.contact_id === contact_id)
					t.assignee = null;
			return withMethods(c);
		},
		setContactPhoto: ({ workspace_id, contact_id, photo }) => {
			const c = contactRow(workspace_id, contact_id);
			c.photo = photo
				? { kind: 'image', photo_media_type: photo.media_type, photo_data: photo.data }
				: { kind: 'none' };
			c.modified = nowSeconds();
			return withMethods(c);
		},
		addContactMethod: ({ workspace_id, contact_id, kind, value }) => {
			contactRow(workspace_id, contact_id);
			return methodReply(addMethod(contact_id, kind, value));
		},
		updateContactMethod: ({ workspace_id, method_id, kind, value }) => {
			requireMethodValue(value);
			const m = contactMethod(workspace_id, method_id);
			Object.assign(m, { kind, value, modified: nowSeconds() });
			touchContact(m.contact_id);
			return methodReply(m);
		},
		deleteContactMethod: ({ workspace_id, method_id }) => {
			const m = contactMethod(workspace_id, method_id);
			state.contactMethods = state.contactMethods.filter((other) => other !== m);
			touchContact(m.contact_id);
			return methodReply(m);
		},

		listTaskFolders: ({ workspace_id }) => liveFolders(workspace_id).map((f) => ({ ...f })),
		createTaskFolder: ({ workspace_id, title }) => {
			workspace(workspace_id);
			const sort_order = Math.max(0, ...liveFolders(workspace_id).map((f) => f.sort_order)) + 1;
			const created = {
				id: nextId(state.folders),
				workspace_id,
				title,
				sort_order,
				deleted: false
			};
			state.folders.push(created);
			return { ...created };
		},
		deleteTaskFolder: ({ workspace_id, folder_id }) => {
			const f = folder(workspace_id, folder_id);
			f.deleted = true;
			// The daemon deletes the folder's tasks with it, and the views that list it.
			for (const t of state.tasks)
				if (t.task_folder_id === folder_id) {
					t.deleted = true;
					keepTask(t, 'deleted');
				}
			state.savedTaskViews = state.savedTaskViews.filter((v) => v.scope !== folder_id);
			return { ...f };
		},
		renameTaskFolder: ({ workspace_id, folder_id, title }) => {
			const f = folder(workspace_id, folder_id);
			f.title = title;
			return { ...f };
		},
		reorderTaskFolders: ({ workspace_id, folder_ids }) => {
			folder_ids.forEach((id, i) => (folder(workspace_id, id).sort_order = i));
			return liveFolders(workspace_id).map((f) => ({ ...f }));
		},
		listSavedTaskViews: ({ workspace_id }) =>
			savedViews(workspace_id).map((v) => structuredClone(v)),
		createSavedTaskView: ({
			workspace_id,
			name,
			scope,
			tags,
			priorities,
			sizes = [],
			sort,
			descending
		}) => {
			workspace(workspace_id);
			ensureViewName(workspace_id, name);
			ensureViewFilters(workspace_id, scope, tags);
			const sort_order = Math.max(0, ...savedViews(workspace_id).map((v) => v.sort_order)) + 1;
			const created: SavedTaskView = {
				id: nextId(state.savedTaskViews),
				workspace_id,
				name,
				scope,
				tags: structuredClone(tags),
				priorities: [...priorities],
				sizes: [...sizes],
				sort,
				descending,
				sort_order
			};
			state.savedTaskViews.push(created);
			return structuredClone(created);
		},
		updateSavedTaskView: ({
			workspace_id,
			view_id,
			scope,
			tags,
			priorities,
			sizes = [],
			sort,
			descending
		}) => {
			const v = savedView(workspace_id, view_id);
			ensureViewFilters(workspace_id, scope, tags);
			Object.assign(v, {
				scope,
				tags: structuredClone(tags),
				priorities: [...priorities],
				sizes: [...sizes],
				sort,
				descending
			});
			return structuredClone(v);
		},
		renameSavedTaskView: ({ workspace_id, view_id, name }) => {
			const v = savedView(workspace_id, view_id);
			ensureViewName(workspace_id, name, view_id);
			v.name = name;
			return structuredClone(v);
		},
		reorderSavedTaskViews: ({ workspace_id, view_ids }) => {
			view_ids.forEach((id, i) => (savedView(workspace_id, id).sort_order = i));
			return savedViews(workspace_id).map((v) => structuredClone(v));
		},
		deleteSavedTaskView: ({ workspace_id, view_id }) => {
			const v = savedView(workspace_id, view_id);
			state.savedTaskViews = state.savedTaskViews.filter((x) => x.id !== view_id);
			return structuredClone(v);
		},
		listTasks: ({
			ids,
			workspace_id,
			folder: filter = 'any',
			statuses,
			priorities = null,
			sizes = null,
			start_date = null,
			due_date = null,
			assignee = null,
			tags = { with: [], without: [] },
			sort = 'created',
			descending = false,
			offset = 0,
			limit = null
		}) => {
			const listed = state.tasks
				.map(withTags)
				.filter(
					(t) =>
						!t.deleted &&
						tags.with.every((id) => t.tag_ids.includes(id)) &&
						!tags.without.some((id) => t.tag_ids.includes(id)) &&
						(priorities == null || priorities.includes(t.priority)) &&
						(sizes == null || (t.size !== null && sizes.includes(t.size))) &&
						(start_date == null || onDay(t.start_date, start_date)) &&
						(due_date == null || onDay(t.due_date, due_date)) &&
						(assignee == null || JSON.stringify(t.assignee) === JSON.stringify(assignee)) &&
						(ids == null || ids.includes(t.id)) &&
						(workspace_id == null || t.workspace_id === workspace_id) &&
						(filter === 'any' ||
							(filter === 'inbox' ? t.task_folder_id === null : t.task_folder_id === filter)) &&
						(statuses == null || statuses.includes(t.status))
				)
				.sort((a, b) => compareTasks(a, b, sort, descending));
			const { rows, total } = paged(listed, offset, limit);
			return { tasks: rows, total };
		},
		createTask: ({
			workspace_id,
			title,
			folder_id = null,
			priority = null,
			size = null,
			assignee = null,
			description = null,
			start_date = null,
			due_date = null,
			tag_ids = []
		}) => {
			workspace(workspace_id);
			if (folder_id !== null) folder(workspace_id, folder_id);
			if (assignee?.kind === 'agent') agent(assignee.agent_id);
			if (assignee?.kind === 'contact') contactRow(workspace_id, assignee.contact_id);
			for (const tag_id of tag_ids) tagOf(workspace_id, tag_id);
			const created: Task = {
				id: nextId(state.tasks),
				workspace_id,
				task_folder_id: folder_id,
				title,
				description: description ?? '',
				status: 'incomplete',
				priority: priority ?? 'none',
				size,
				assignee,
				start_date,
				due_date,
				deleted: false,
				tag_ids: []
			};
			state.tasks.push(created);
			keepTask(created, 'created');
			for (const tag_id of tag_ids) state.tagged.push({ tag_id, task_id: created.id });
			return withTags(created);
		},
		// Only the tasks whose status changed come back.
		changeTasksStatus: ({ workspace_id, task_ids, status }) =>
			task_ids
				.map((id) => task(workspace_id, id))
				.filter((t) => t.status !== status)
				.map((t) => {
					t.status = status;
					keepTask(t, 'edited');
					return { ...t };
				}),
		renameTask: ({ workspace_id, task_id, title }) => {
			const t = task(workspace_id, task_id);
			t.title = title;
			keepTask(t, 'edited');
			return { ...t };
		},
		setTaskPriority: ({ workspace_id, task_id, priority }) => {
			const t = task(workspace_id, task_id);
			t.priority = priority;
			return { ...t };
		},
		setTaskSize: ({ workspace_id, task_id, size }) => {
			const t = task(workspace_id, task_id);
			t.size = size;
			return { ...t };
		},
		setTaskAssignee: ({ workspace_id, task_id, assignee }) => {
			const t = task(workspace_id, task_id);
			if (assignee?.kind === 'agent') agent(assignee.agent_id);
			if (assignee?.kind === 'contact') contactRow(workspace_id, assignee.contact_id);
			t.assignee = assignee;
			return { ...t };
		},
		editTaskDescription: ({ workspace_id, task_id, description }) => {
			const t = task(workspace_id, task_id);
			t.description = description;
			keepTask(t, 'edited');
			return { ...t };
		},
		scheduleTask: ({ workspace_id, task_id, start_date, due_date }) => {
			const t = task(workspace_id, task_id);
			t.start_date = start_date;
			t.due_date = due_date;
			return { ...t };
		},
		moveTask: ({ workspace_id, task_id, folder_id = null }) => {
			const t = task(workspace_id, task_id);
			if (folder_id !== null) folder(workspace_id, folder_id);
			t.task_folder_id = folder_id;
			return { ...t };
		},
		deleteTask: ({ workspace_id, task_id }) => {
			const t = task(workspace_id, task_id);
			t.deleted = true;
			keepTask(t, 'deleted');
			return { ...t };
		},

		// Like the daemon's index for the queries the UI sends (`"word"*` per
		// word, all required): every word starts a word of the title or
		// contents, title matches first.
		searchWorkspace: ({ workspace_id, query }) => {
			const words = searchWords(query);
			if (words.length === 0) return [];
			const hits: { title: boolean; result: SearchResult }[] = [];
			const consider = (
				entity_kind: SearchResult['entity_kind'],
				entity_id: number,
				title: string,
				contents: string
			) => {
				const [inTitle, inContents] = [starts(title), starts(contents)];
				if (!words.every((w) => inTitle(w) || inContents(w))) return;
				hits.push({
					title: words.some(inTitle),
					result: { id: 0, entity_kind, entity_id, workspace_id, title, contents }
				});
			};
			for (const d of state.documents)
				if (d.workspace_id === workspace_id && !d.deleted)
					consider('document', d.id, d.title, d.contents);
			for (const t of state.tasks)
				if (t.workspace_id === workspace_id && !t.deleted) consider('task', t.id, t.title, '');
			// JSON resources are indexed by title only.
			for (const a of state.resources)
				if (a.workspace_id === workspace_id)
					consider('resource', a.id, a.title, a.content_kind === 'json' ? '' : a.contents);
			return hits
				.sort((a, b) => Number(b.title) - Number(a.title))
				.map(({ result }, index) => ({ ...result, id: index + 1 }));
		},
		documentation: () => ({
			documentation: '// Mock documentation.\ndeclare const cohesion: {};\n'
		}),
		runScript: ({ script }) => JSON.stringify({ ran: script.length }),
		renderMermaid: ({ source }) =>
			/^\s*(flowchart|graph|sequenceDiagram)\b/.test(source)
				? {
						kind: 'svg',
						svg: '<svg xmlns="http://www.w3.org/2000/svg" class="pele" viewBox="0 0 120 40" width="120" height="40" style="max-width:100%;height:auto"><text x="60" y="24" text-anchor="middle" fill="var(--pele-fg,#1f1f1f)">Mock diagram</text></svg>',
						width: 120,
						height: 40
					}
				: { kind: 'refused', reason: 'syntax', message: 'Mock: this is not a diagram.' },

		updateModelCatalog: () => null,
		listIntelligenceEndpoints: () => state.endpoints.map((e) => structuredClone(e)),
		createIntelligenceEndpoint: (args) => {
			if (args.preset !== 'custom' && !args.api_key)
				throw new ProtocolError('validationFailed', 'This endpoint requires an API key.');
			const defaults =
				args.preset === 'custom'
					? {
							name: args.provider_identifier,
							provider_identifier: args.provider_identifier,
							dialect: args.dialect,
							default_model: null,
							base_url: args.base_url,
							usage_url: null,
							cache_minutes: 5,
							long_cache: null
						}
					: PRESET_DEFAULTS[args.preset];
			const name = args.name ?? defaults.name;
			if (state.endpoints.some((e) => e.name.toLowerCase() === name.toLowerCase()))
				throw new ProtocolError(
					'validationFailed',
					`An endpoint named ${name} already exists; give this one another name.`,
					'name'
				);
			const created: RedactedIntelligenceEndpoint = {
				id: nextId(state.endpoints),
				name,
				backend: {
					kind: 'integrated',
					provider_identifier: defaults.provider_identifier,
					dialect: defaults.dialect,
					base_url: defaults.base_url,
					usage_url: args.usage_url ?? defaults.usage_url,
					cache_minutes: args.cache_minutes ?? defaults.cache_minutes,
					long_cache: args.long_cache ?? defaults.long_cache
				}
			};
			state.endpoints.push(created);
			if (defaults.default_model !== null)
				addEndpointModel({ endpoint_id: created.id, model_identifier: defaults.default_model });
			return structuredClone(created);
		},
		deleteIntelligenceEndpoint: ({ id }) => {
			const index = state.endpoints.findIndex((e) => e.id === id);
			if (index < 0) notFound('intelligence endpoint');
			const endpoint = structuredClone(state.endpoints[index]);
			dropModels((m) => m.endpoint_id === id);
			state.endpoints.splice(index, 1);
			return endpoint;
		},
		setIntelligenceEndpointUsageUrl: ({ id, usage_url }) => {
			const endpoint =
				state.endpoints.find((e) => e.id === id) ?? notFound('intelligence endpoint');
			endpoint.backend = { ...endpoint.backend, usage_url };
			return structuredClone(endpoint);
		},
		setIntelligenceEndpointCacheMinutes: ({ id, cache_minutes }) => {
			const endpoint =
				state.endpoints.find((e) => e.id === id) ?? notFound('intelligence endpoint');
			endpoint.backend = { ...endpoint.backend, cache_minutes };
			return structuredClone(endpoint);
		},
		setIntelligenceEndpointLongCache: ({ id, long_cache }) => {
			const endpoint =
				state.endpoints.find((e) => e.id === id) ?? notFound('intelligence endpoint');
			endpoint.backend = { ...endpoint.backend, long_cache: long_cache ?? null };
			return structuredClone(endpoint);
		},
		addEndpointModel,
		removeEndpointModel: ({ id }) => {
			const model = state.endpointModels.find((m) => m.id === id) ?? notFound('endpoint model');
			dropModels((m) => m.id === id);
			return { ...model };
		},
		listEndpointModels: ({ endpoint_id }) => modelsOf(endpoint_id).map((m) => ({ ...m })),
		setIntelligenceDefault: ({ tier, endpoint_model_id }) => {
			if (!state.endpointModels.some((m) => m.id === endpoint_model_id))
				throw new ProtocolError('notFound', 'This operation contains an invalid reference.');
			const existing = state.defaults.find((d) => d.tier === tier);
			if (existing) existing.endpoint_model_id = endpoint_model_id;
			else state.defaults.push({ id: nextId(state.defaults), tier, endpoint_model_id });
			return null;
		},
		clearIntelligenceDefault: ({ tier }) => {
			state.defaults = state.defaults.filter((d) => d.tier !== tier);
			return null;
		},
		listCatalogModels: ({ provider_identifier, query }) => {
			const providers =
				provider_identifier == null
					? new Set(
							state.endpoints.flatMap((e) =>
								e.backend.kind === 'integrated' ? [e.backend.provider_identifier] : []
							)
						)
					: new Set([provider_identifier]);
			const q = query?.toLowerCase() ?? '';
			return state.catalog
				.filter(
					(m) =>
						providers.has(m.provider_identifier) &&
						(m.model_identifier.toLowerCase().includes(q) || m.model_name.toLowerCase().includes(q))
				)
				.sort(
					(a, b) =>
						a.provider_identifier.localeCompare(b.provider_identifier) ||
						a.model_identifier.localeCompare(b.model_identifier)
				)
				.map((m) => ({ ...m }));
		},
		listIntelligenceSpend: () => spend(state),
		setIntelligenceSpendLimit: ({ period, amount }) => {
			if (!(amount > 0)) throw new ProtocolError('validationFailed', 'Validation failed.');
			const existing = state.spendLimits.find((l) => l.period === period);
			if (existing) existing.amount = amount;
			else state.spendLimits.push({ period, amount });
			return spend(state);
		},
		clearIntelligenceSpendLimit: ({ period }) => {
			state.spendLimits = state.spendLimits.filter((l) => l.period !== period);
			return spend(state);
		},
		listIntelligenceCostEvents: ({ limit }) =>
			[...state.costEvents]
				.reverse()
				.slice(0, limit)
				.map((e) => ({ ...e })),
		getIntelligenceThread: ({ thread_id }) => {
			const thread =
				state.threads.find((t) => t.id === thread_id) ?? notFound('intelligence thread');
			return {
				thread: { ...thread },
				messages: state.messages.filter((m) => m.thread_id === thread_id).map((m) => ({ ...m })),
				cost_events: state.costEvents
					.filter((e) => e.thread_id === thread_id)
					.map((e) => ({ ...e }))
			};
		},

		listExtensionPermissions: ({ extension_id }) =>
			state.permissions.filter((p) => p.extension_id === extension_id).map((p) => ({ ...p })),
		setExtensionPermission: ({ extension_id, permission, enabled }) => {
			const row = state.permissions.find(
				(p) => p.extension_id === extension_id && p.permission === permission
			);
			if (row) row.enabled = enabled;
			else
				state.permissions.push({
					id: nextId(state.permissions),
					extension_id,
					permission,
					enabled
				});
			return null;
		},
		listExtensionHooks: ({ extension_id }) =>
			state.hooks.filter((h) => h.extension_id === extension_id).map((h) => ({ ...h })),
		setExtensionHook: ({ extension_id, hook, enabled }) => {
			const row = state.hooks.find((h) => h.extension_id === extension_id && h.hook === hook);
			if (row) row.enabled = enabled;
			else state.hooks.push({ id: nextId(state.hooks), extension_id, hook, enabled });
			return null;
		},
		uninstallExtension: ({ extension_id }) => {
			state.extensions = state.extensions.filter((e) => e.id !== extension_id);
			state.permissions = state.permissions.filter((p) => p.extension_id !== extension_id);
			state.hooks = state.hooks.filter((h) => h.extension_id !== extension_id);
			orphan((a) => a.host.kind === 'extension' && a.host.extension_id === extension_id);
			return null;
		},
		// Installs a script `previewExtension` served the way the daemon does:
		// new permissions and hooks start off, existing grants stay, dropped
		// ones go. A pasted script the mock can't read is accepted and ignored.
		registerExtension: ({ script, allow_overwrite }) => {
			const source = [...state.sources.values()].find((s) => s.script === script);
			if (!source) return null;
			const { manifest } = source;
			let extension = state.extensions.find((e) => e.script_identifier === manifest.identifier);
			if (extension && !allow_overwrite)
				throw new ProtocolError(
					'validationFailed',
					'This extension is already installed. You must override to allow this.'
				);
			if (!extension) {
				extension = {
					id: nextId(state.extensions),
					name: '',
					version: 0,
					script_identifier: manifest.identifier,
					priority: 0,
					description: '',
					author: '',
					icon: null
				};
				state.extensions.push(extension);
			}
			Object.assign(extension, {
				name: manifest.name,
				version: manifest.version,
				description: manifest.description ?? '',
				author: manifest.author ?? '',
				icon: manifest.icon
			});
			const id = extension.id;
			state.permissions = state.permissions.filter(
				(p) => p.extension_id !== id || manifest.permissions.includes(p.permission)
			);
			for (const permission of manifest.permissions)
				if (!state.permissions.some((p) => p.extension_id === id && p.permission === permission))
					state.permissions.push({
						id: nextId(state.permissions),
						extension_id: id,
						permission,
						enabled: false
					});
			state.hooks = state.hooks.filter(
				(h) => h.extension_id !== id || manifest.hooks.includes(h.hook)
			);
			for (const hook of manifest.hooks)
				if (!state.hooks.some((h) => h.extension_id === id && h.hook === hook))
					state.hooks.push({ id: nextId(state.hooks), extension_id: id, hook, enabled: false });
			return null;
		},
		// Each registry the user granted `registry` answers; matching is the
		// registry's business, so the mock's match name, author and description.
		searchExtensionRegistries: ({ query }) => {
			const q = query.trim().toLowerCase();
			return grantedRegistries().map((e) => {
				const registry = state.registries.find((r) => r.extension_id === e.id);
				return {
					extension_id: e.id,
					name: e.name,
					results: (registry?.results ?? [])
						.filter(({ manifest: m }) =>
							[m.name, m.author ?? '', m.description ?? ''].some((text) =>
								text.toLowerCase().includes(q)
							)
						)
						.map((r) => structuredClone(r)),
					error: registry?.error ?? null
				};
			});
		},
		// The mock checks on every call, so listing answers what checking would.
		listExtensionUpdates: () => extensionUpdates(),
		checkExtensionUpdates: () => extensionUpdates(),
		previewExtension: ({ source_url }) => {
			const source = state.sources.get(source_url);
			if (!source)
				throw new ProtocolError('validationFailed', `${source_url} answered 404 Not Found.`);
			return preview(source);
		},
		// Only scripts the mock serves have a manifest it can read.
		previewExtensionScript: ({ script }) => {
			const source = [...state.sources.values()].find((s) => s.script === script);
			if (!source)
				throw new ProtocolError('validationFailed', 'The mock can only read scripts it serves.');
			return preview(source);
		},
		// No mock extension defines `serviceRequest`.
		callExtension: ({ identifier }) => {
			const extension =
				state.extensions.find((e) => e.script_identifier === identifier) ?? notFound('extension');
			if (!granted(extension.id, 'service'))
				throw new ProtocolError(
					'validationFailed',
					"This extension doesn't have the `service` permission enabled."
				);
			throw new ProtocolError(
				'validationFailed',
				`${extension.name}: script does not define a function named \`serviceRequest\``
			);
		},

		// Each extension granted `oauth` describes itself; a failing one is
		// named after the extension and carries the error.
		listConnectorServices: () =>
			state.extensions
				.filter((e) => granted(e.id, 'oauth'))
				.map((e) => {
					const c = state.connectors.find((x) => x.extension_id === e.id);
					const error = c ? c.error : `${e.name}: serviceDescription is not defined`;
					return {
						extension_id: e.id,
						name: e.name,
						title: c && !error ? c.title : e.name,
						unit: c && !error ? c.unit : '',
						icon: e.icon,
						error,
						sign_in_hint: null
					};
				}),
		beginConnectorAuthentication: ({ extension_id }) => {
			connector(extension_id, ['oauth']);
			return { url: `https://example.com/connect/${extension_id}/authorize` };
		},
		// Any code signs in to the service's one account; adding it again replaces it.
		completeConnectorAuthentication: ({ extension_id, code }) => {
			const c = connector(extension_id, ['oauth']);
			if (code.trim() === '')
				throw new ProtocolError('upstreamError', `${c.title}: the code was refused.`);
			const extension = state.extensions.find((e) => e.id === extension_id)!;
			// The definition is stored the first time, as the daemon does.
			let stored = state.definitions.find((d) => d.identifier === extension.script_identifier);
			if (!stored) {
				stored = {
					id: nextId(state.definitions),
					identifier: extension.script_identifier,
					version: 1,
					title: c.title,
					icon: extension.icon,
					unit: c.unit,
					kinds: [],
					metadata_keys: [],
					created_at: nowSeconds()
				};
				const id = stored.id;
				let kindId = Math.max(0, ...state.definitions.flatMap((d) => d.kinds.map((k) => k.id)));
				stored.kinds = c.kinds.map(([identifier, title, plural]) => ({
					id: ++kindId,
					connector_id: id,
					identifier,
					title,
					plural,
					contents: ''
				}));
				state.definitions.push(stored);
			}
			let account = state.connectorAccounts.find(
				(a) =>
					a.host.kind === 'extension' &&
					a.host.extension_id === extension_id &&
					a.host.identifier === c.account.identifier
			);
			if (!account) {
				account = {
					id: nextId(state.connectorAccounts),
					connector_id: stored.id,
					host: { kind: 'extension', extension_id, identifier: c.account.identifier },
					description: '',
					credential_failed_at: null,
					created_at: nowSeconds()
				};
				state.connectorAccounts.push(account);
			}
			account.description = c.account.description;
			// A new credential clears the flag, as the daemon does.
			account.credential_failed_at = null;
			return redacted(account);
		},
		listConnectorAccounts: () =>
			state.connectorAccounts
				.map(redacted)
				.sort(
					(a, b) =>
						a.description.localeCompare(b.description, undefined, { sensitivity: 'base' }) ||
						a.connector.title.localeCompare(b.connector.title, undefined, { sensitivity: 'base' })
				),
		deleteConnectorAccount: ({ account_id }) => {
			const account = redacted(connectorAccount(account_id));
			state.connectorAccounts = state.connectorAccounts.filter((a) => a.id !== account_id);
			state.resources = state.resources.filter((r) => r.account_id !== account_id);
			return account;
		},
		// Within one connector; the target's copy of an item both have stays.
		migrateResources: ({ from_account_id, to_account_id }) => {
			const from = connectorAccount(from_account_id);
			const to = connectorAccount(to_account_id);
			if (from.connector_id !== to.connector_id)
				throw new ProtocolError('notFound', 'This operation contains an invalid reference.');
			const held = (r: Resource) =>
				state.resources.some(
					(t) =>
						t.account_id === to_account_id &&
						t.workspace_id === r.workspace_id &&
						t.identifier === r.identifier
				);
			state.resources = state.resources.filter((r) => r.account_id !== from_account_id || !held(r));
			for (const r of state.resources)
				if (r.account_id === from_account_id) r.account_id = to_account_id;
			return null;
		},
		listConnectorOrganizationUnits: ({ account_id }) => {
			const account = connectorAccount(account_id);
			served(account);
			if (account.host.kind !== 'extension') throw new Error('served() refuses an orphan');
			return connector(account.host.extension_id).units.map((u) => ({ ...u }));
		},
		searchConnectorResources: ({
			account_id,
			workspace_id,
			kind_id,
			organization_unit_id,
			search
		}) => {
			workspace(workspace_id);
			const account = connectorAccount(account_id);
			served(account);
			const kind = definition(account.connector_id).kinds.find((k) => k.id === kind_id);
			if (!kind) throw new ProtocolError('validationFailed', 'No such kind.');
			const q = search?.toLowerCase() ?? '';
			const { host } = account;
			if (host.kind !== 'extension') throw new Error('served() refuses an orphan');
			return connector(host.extension_id)
				.resources.filter((r) => !organization_unit_id || r.unit === organization_unit_id)
				.filter((r) => r.kind_identifier === kind.identifier)
				.filter((r) => r.title.toLowerCase().includes(q))
				.map((r) => ({
					identifier: r.identifier,
					title: r.title,
					// A date the service doesn't have is null, as the daemon sends it.
					last_updated: r.last_updated || null,
					imported: state.resources.some(
						(a) =>
							a.workspace_id === workspace_id &&
							a.account_id === account_id &&
							a.identifier === r.identifier
					)
				}));
		},
		// Upserts by (workspace, account, identifier); `kind` is prefixed with
		// the connector's identifier, as a read of it is.
		importResource: ({ account_id, workspace_id, identifier }) => {
			workspace(workspace_id);
			const account = connectorAccount(account_id);
			const remote = remoteResource(account, identifier);
			const kind = definition(account.connector_id).kinds.find(
				(k) => k.identifier === remote.kind_identifier
			);
			if (!kind)
				throw new ProtocolError(
					'validationFailed',
					`The connector doesn’t declare the kind "${remote.kind_identifier}".`
				);
			let imported = state.resources.find(
				(a) =>
					a.workspace_id === workspace_id &&
					a.account_id === account_id &&
					a.identifier === identifier
			);
			if (!imported) {
				imported = {
					id: nextId(state.resources),
					workspace_id,
					account_id,
					identifier,
					kind_id: kind.id,
					kind: '',
					title: '',
					content_kind: remote.content_kind,
					contents: '',
					last_updated: 0,
					modified: 0
				};
				state.resources.push(imported);
			}
			Object.assign(imported, {
				kind_id: kind.id,
				kind: remote.kind,
				title: remote.title,
				content_kind: remote.content_kind,
				contents: remote.contents,
				last_updated: remote.last_updated,
				modified: nowSeconds()
			});
			return { ...imported };
		},
		// The mock keeps no metadata, so a metadata sort leaves the newest import first.
		listResources: ({ workspace_id, kind, search, sort, descending }) => {
			workspace(workspace_id);
			const q = search?.toLowerCase() ?? '';
			return state.resources
				.filter(
					(a) =>
						a.workspace_id === workspace_id &&
						(kind == null || a.kind === kind) &&
						a.title.toLowerCase().includes(q)
				)
				.sort((a, b) => {
					const order =
						sort === 'title'
							? a.title.localeCompare(b.title, undefined, { sensitivity: 'base' })
							: sort === 'lastUpdated'
								? a.last_updated - b.last_updated
								: 0;
					return (descending ? -order : order) || b.id - a.id;
				})
				.map((a) => ({ ...a }));
		},
		getResource: ({ workspace_id, resource_id }) => ({ ...resource(workspace_id, resource_id) }),
		// Like the daemon, a resource whose connector can't draw it has no view.
		renderResource: ({ workspace_id, resource_id }) => {
			const found = resource(workspace_id, resource_id);
			const { host } = connectorAccount(found.account_id);
			if (host.kind !== 'extension' || !granted(host.extension_id, 'connector')) return null;
			const service = state.connectors.find((c) => c.extension_id === host.extension_id);
			const remote = service?.resources.find((r) => r.identifier === found.identifier);
			return remote?.render ? structuredClone(remote.render) : null;
		},
		deleteResource: ({ workspace_id, resource_id }) => {
			const removed = resource(workspace_id, resource_id);
			state.resources = state.resources.filter((a) => a.id !== resource_id);
			return { ...removed };
		},
		listArticles: ({ filter, offset = 0, limit = null }) => {
			const covered = coveredBy(filter);
			const listed = state.articles
				.filter((l) => !l.deleted && covered(l))
				.sort((a, b) => b.published_at - a.published_at || b.id - a.id)
				.map(summary);
			const { rows, total } = paged(listed, offset, limit);
			return { articles: rows, total };
		},
		setArticleRead: ({ article_id, read }) => {
			const found = article(article_id);
			found.read = read;
			return structuredClone(found);
		},
		markArticlesRead: ({ filter }) => {
			const covered = coveredBy(filter);
			let marked = 0;
			for (const l of state.articles)
				if (!l.deleted && !l.read && covered(l)) {
					l.read = true;
					marked++;
				}
			return { marked };
		},
		listFeeds: () => liveFeeds().map(feedSummary),
		// The mock can't read the web: every address reads as a feed of one item.
		addFeed: ({ url, full_page_policy = 'whenExcerpt' }) => {
			let parsed: URL;
			try {
				parsed = new URL(url.trim());
			} catch {
				parsed = new URL('about:blank');
			}
			if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:')
				throw new ProtocolError(
					'validationFailed',
					'Enter a feed address starting with http:// or https://.',
					'url'
				);
			if (liveFeeds().some((f) => f.url === parsed.href))
				throw new ProtocolError('validationFailed', 'You already follow that feed.', 'url');
			const now = nowSeconds();
			const created: Feed = {
				id: nextId(state.feeds),
				deleted: false,
				url: parsed.href,
				title: parsed.host,
				sort_order: Math.max(0, ...state.feeds.map((f) => f.sort_order)) + 1,
				full_page_policy,
				status: { state: 'refreshed', at: now, added: 0 },
				created_at: now
			};
			state.feeds.push(created);
			const itemURL = new URL('/welcome', parsed).href;
			state.articles.push(
				feedItem(
					nextId(state.articles),
					created.id,
					parsed.host,
					itemURL,
					`Welcome to ${parsed.host}`,
					now,
					now,
					'The first post.'
				)
			);
			created.status = { state: 'refreshed', at: now, added: 1 };
			return structuredClone(created);
		},
		refreshFeed: ({ feed_id }) => {
			const f = feed(feed_id);
			f.status = { state: 'refreshed', at: nowSeconds(), added: 0 };
			return structuredClone(f);
		},
		refreshFeeds: () => {
			const now = nowSeconds();
			return liveFeeds().map((f) => {
				f.status = { state: 'refreshed', at: now, added: 0 };
				return structuredClone(f);
			});
		},
		renameFeed: ({ feed_id, title }) => {
			requireTitle(title);
			const f = feed(feed_id);
			f.title = title;
			return structuredClone(f);
		},
		reorderFeeds: ({ feed_ids }) => {
			feed_ids.forEach((id, i) => (feed(id).sort_order = i));
			return liveFeeds().map(feedSummary);
		},
		setFeedFullPagePolicy: ({ feed_id, full_page_policy }) => {
			const f = feed(feed_id);
			f.full_page_policy = full_page_policy;
			return structuredClone(f);
		},
		deleteFeed: ({ feed_id }) => {
			const f = feed(feed_id);
			f.deleted = true;
			removeItems([feed_id]);
			return structuredClone(f);
		},
		pruneFeeds: ({ feed_ids, before }) => ({ removed: removeItems(feed_ids, before) }),
		// Every word must start a word of the title, author or contents.
		searchFeedCorpus: ({ query, feed_ids = null, offset = 0, limit = null }) => {
			const words = (query.match(/[\p{L}\p{N}]+/gu) ?? []).map((w) => w.toLowerCase());
			if (words.length === 0) return { hits: [], total: 0 };
			const hits: FeedCorpusHit[] = [];
			for (const l of state.articles) {
				if (l.deleted) continue;
				if (feed_ids !== null && !feed_ids.includes(l.feed_id)) continue;
				const tokens =
					`${l.title} ${l.author ?? ''} ${l.contents}`.toLowerCase().match(/[\p{L}\p{N}]+/gu) ?? [];
				if (!words.every((w) => tokens.some((t) => t.startsWith(w)))) continue;
				const { id, feed_id, read, url, title, author, published_at, published_precision } = l;
				hits.push({
					id,
					feed_id,
					read,
					url,
					title,
					author,
					published_at,
					published_precision,
					excerpt: l.contents.slice(0, 160)
				});
			}
			const { rows, total } = paged(hits, offset, limit);
			return { hits: rows, total };
		},
		listTags: ({ workspace_id }) => {
			workspace(workspace_id);
			return tagsOf(workspace_id);
		},
		reorderTags: ({ workspace_id, tag_ids }) => {
			workspace(workspace_id);
			tag_ids.forEach((id, i) => (tagOf(workspace_id, id).sort_order = i));
			return tagsOf(workspace_id);
		},
		createTag: ({ workspace_id, name }) => {
			workspace(workspace_id);
			ensureTagName(workspace_id, name);
			const created: Tag = {
				id: nextId(state.tags),
				workspace_id,
				name,
				sort_order:
					Math.max(
						0,
						...state.tags.filter((t) => t.workspace_id === workspace_id).map((t) => t.sort_order)
					) + 1,
				created_at: nowSeconds()
			};
			state.tags.push(created);
			return { ...created };
		},
		renameTag: ({ workspace_id, tag_id, name }) => {
			const tag = tagOf(workspace_id, tag_id);
			ensureTagName(workspace_id, name, tag_id);
			tag.name = name;
			return { ...tag };
		},
		deleteTag: ({ workspace_id, tag_id }) => {
			const tag = tagOf(workspace_id, tag_id);
			state.tags = state.tags.filter((t) => t.id !== tag_id);
			state.tagged = state.tagged.filter((x) => x.tag_id !== tag_id);
			// The daemon takes it out of every saved view too.
			for (const v of state.savedTaskViews) {
				v.tags.with = v.tags.with.filter((id) => id !== tag_id);
				v.tags.without = v.tags.without.filter((id) => id !== tag_id);
			}
			return { ...tag };
		},
		setTagged: ({ workspace_id, tag_id, task_id, tagged }) => {
			tagOf(workspace_id, tag_id);
			const found = state.tasks.find((t) => t.id === task_id && t.workspace_id === workspace_id);
			if (!found)
				throw new ProtocolError('notFound', 'This operation contains an invalid reference.');
			state.tagged = state.tagged.filter((x) => !(x.tag_id === tag_id && x.task_id === task_id));
			if (tagged) state.tagged.push({ tag_id, task_id });
			return withTags(found);
		},
		getArticle: ({ article_id }) => structuredClone(article(article_id)),
		resolveLink: ({ link }) => structuredClone(resolveLink(link)),
		backlinksOf: ({ link }) => {
			const target = parseItemLink(link.trim());
			if (!target)
				throw new ProtocolError(
					'validationFailed',
					'That isn’t a link to a Cohesion item, such as cohesion://task/3/42.',
					'link'
				);
			const linking: Embed[] = [];
			for (const d of state.documents) {
				const self = target.kind === 'document' && d.id === target.id;
				if (d.deleted || self || !linksTo(d.contents, target)) continue;
				try {
					linking.push(structuredClone(resolveLink(`cohesion://document/${d.id}`)));
				} catch (error) {
					if (!(error instanceof ProtocolError && error.code === 'notFound')) throw error;
				}
			}
			return linking.sort((a, b) => a.title.toLowerCase().localeCompare(b.title.toLowerCase()));
		},
		resolveEmbeds: ({ text }) =>
			textPieces(text).map((piece) => {
				if (piece.kind === 'markdown') return piece;
				const { href: link, label } = piece;
				try {
					return { kind: 'embed', link, label, embed: structuredClone(resolveLink(link)) };
				} catch (error) {
					if (error instanceof ProtocolError && error.code === 'notFound')
						return { kind: 'embed', link, label, embed: null };
					throw error;
				}
			}),
		resnapshotArticle: ({ article_id }) => {
			const found = article(article_id);
			return record(found, new URL(found.url));
		},
		renameArticle: ({ article_id, title }) => {
			requireTitle(title);
			const found = article(article_id);
			found.title = title;
			found.modified = nowSeconds();
			return structuredClone(found);
		},
		listWorkspaceResourceKinds: ({ workspace_id }) => {
			workspace(workspace_id);
			return shownKinds(workspace_id);
		},
		setResourceKindShown: ({ workspace_id, kind_id, shown }) => {
			workspace(workspace_id);
			const inWorkspace = state.shownKinds.filter((k) => k.workspace_id === workspace_id);
			const at = state.shownKinds.findIndex(
				(k) => k.workspace_id === workspace_id && k.kind_id === kind_id
			);
			// Like the daemon, a kind shown later goes last.
			if (shown && at === -1)
				state.shownKinds.push({
					id: nextId(state.shownKinds),
					workspace_id,
					kind_id,
					sort_order: Math.max(0, ...inWorkspace.map((k) => k.sort_order)) + 1
				});
			if (!shown && at !== -1) state.shownKinds.splice(at, 1);
			return shownKinds(workspace_id);
		},
		reorderWorkspaceResourceKinds: ({ workspace_id, kind_ids }) => {
			workspace(workspace_id);
			kind_ids.forEach((kind_id, i) => {
				const shown = state.shownKinds.find(
					(k) => k.workspace_id === workspace_id && k.kind_id === kind_id
				);
				(shown ?? notFound('kind')).sort_order = i;
			});
			return shownKinds(workspace_id);
		},
		// Takes the service's copy of each resource whose `last_updated` moved.
		refreshResources: ({ workspace_id }) => {
			workspace(workspace_id);
			const summary = { checked: 0, updated: 0, failed: 0 };
			// Like the daemon, a resource counts as checked only when its fetch
			// succeeds, and an orphaned account's are skipped.
			for (const a of state.resources.filter((x) => x.workspace_id === workspace_id)) {
				const account = connectorAccount(a.account_id);
				if (account.host.kind === 'orphaned') continue;
				try {
					const remote = remoteResource(account, a.identifier);
					summary.checked++;
					if (remote.last_updated === a.last_updated) continue;
					Object.assign(a, {
						title: remote.title,
						contents: remote.contents,
						content_kind: remote.content_kind,
						last_updated: remote.last_updated,
						modified: nowSeconds()
					});
					summary.updated++;
				} catch {
					summary.failed++;
				}
			}
			return summary;
		},

		listIntelligenceAgents: () => sortedAgents().map(agentRow),
		reorderIntelligenceAgents: ({ agent_ids }) => {
			agent_ids.forEach((id, i) => (agent(id).sort_order = i));
			return sortedAgents().map(agentRow);
		},
		setAgentPinned: ({ agent_id, pinned }) => {
			const a = agent(agent_id);
			if (pinned && !a.pinned)
				a.sort_order =
					Math.max(0, ...state.agents.filter((o) => o.pinned).map((o) => o.sort_order)) + 1;
			a.pinned = pinned;
			return agentRow(a);
		},
		getIntelligenceAgent: ({ agent_id }) => agentRow(agent(agent_id)),
		setAgentTeam: ({ agent_id, team_id }) => {
			const a = agent(agent_id);
			if (team_id != null) agentTeam(team_id);
			a.team_id = team_id ?? null;
			return agentRow(a);
		},
		listAgentTeams: () => sortedTeams(),
		createAgentTeam: ({ title }) => {
			const created: IntelligenceAgentTeam = {
				id: nextId(state.agentTeams),
				title: teamTitle(title)
			};
			state.agentTeams.push(created);
			return { ...created };
		},
		renameAgentTeam: ({ team_id, title }) => {
			const t = agentTeam(team_id);
			t.title = teamTitle(title, team_id);
			return { ...t };
		},
		deleteAgentTeam: ({ team_id }) => {
			const t = agentTeam(team_id);
			state.agentTeams = state.agentTeams.filter((x) => x.id !== team_id);
			for (const a of state.agents) if (a.team_id === team_id) a.team_id = null;
			state.memories = state.memories.filter(
				(m) => m.scope.kind !== 'team' || m.scope.team_id !== team_id
			);
			return { ...t };
		},
		createIntelligenceAgent: ({
			name,
			color,
			emoji,
			personality,
			verbosity,
			role,
			directive,
			team_id
		}) => {
			const created: IntelligenceAgent = {
				id: nextId(state.agents),
				created_at: nowSeconds(),
				// The daemon picks an unused first name; the mock numbers them.
				agent_name: name?.trim() || `Agent ${state.agents.length + 1}`,
				agent_color: color,
				agent_emoji: emojiValue(emoji),
				agent_personality: personality ?? 'balanced',
				agent_verbosity: verbosity ?? 'brief',
				agent_role: role?.trim() ?? '',
				agent_directive: directive?.trim() ?? '',
				activity: null,
				foreground_started: null,
				context_tokens: null,
				agent_unread: false,
				last_message_at: null,
				pinned: false,
				sort_order: Math.max(0, ...state.agents.map((a) => a.sort_order)) + 1,
				team_id: team_id ?? null,
				following_count: 0
			};
			state.agents.push(created);
			return { ...created };
		},
		updateIntelligenceAgent: ({
			agent_id,
			name,
			color,
			emoji,
			personality,
			verbosity,
			role,
			directive
		}) => {
			const a = agent(agent_id);
			if (name != null) {
				if (name.trim() === '')
					throw new ProtocolError('validationFailed', 'Name can’t be empty.', 'name');
				a.agent_name = name;
			}
			if (color != null) a.agent_color = color;
			if (emoji != null) a.agent_emoji = emojiValue(emoji);
			if (personality != null) a.agent_personality = personality;
			if (verbosity != null) a.agent_verbosity = verbosity;
			if (role != null) a.agent_role = role.trim();
			if (directive != null) a.agent_directive = directive.trim();
			return agentRow(a);
		},
		deleteIntelligenceAgent: ({ agent_id }) => {
			const a = agent(agent_id);
			state.agents = state.agents.filter((x) => x.id !== agent_id);
			for (const t of state.tasks)
				if (t.assignee?.kind === 'agent' && t.assignee.agent_id === agent_id) t.assignee = null;
			state.agentMessages = state.agentMessages.filter((m) => m.agent_id !== agent_id);
			state.agentCues = state.agentCues.filter((c) => c.agent_id !== agent_id);
			state.agentRuns = state.agentRuns.filter((r) => r.agent_id !== agent_id);
			state.memories = state.memories.filter(
				(m) => m.scope.kind !== 'agent' || m.scope.agent_id !== agent_id
			);
			return { ...a };
		},
		markAgentRead: ({ agent_id }) => {
			const a = agent(agent_id);
			a.agent_unread = false;
			return agentRow(a);
		},
		listAgentMessages: ({ agent_id, before, limit }) => {
			agent(agent_id);
			const older = state.agentMessages
				.filter((m) => m.agent_id === agent_id && (before == null || m.id < before))
				.sort((a, b) => a.id - b.id);
			return older.slice(Math.max(0, older.length - limit)).map((m) => ({ ...m }));
		},
		getAgentMessage: ({ message_id }) => ({ ...agentMessageById(message_id) }),
		setAgentMessageReaction: ({ message_id, reaction }) => {
			const m = agentMessageById(message_id);
			if (m.sender === 'user')
				throw new ProtocolError('validationFailed', 'React to the agent’s messages, not your own.');
			m.reaction = reaction?.trim() || null;
			m.reacted_at = nowSeconds();
			Object.assign(m, reactionExcerpt(m.reaction, m.excerpt));
			return null;
		},
		listAgentMessageAttachments: ({ message_id }) =>
			state.agentAttachments
				.filter((a) => a.message.kind === 'agent' && a.message.agent_message_id === message_id)
				.map((a) => ({ ...a })),
		// The agent answers with a canned reply; it runs no code here. The
		// reply is one model call, on a thread of its own.
		sendAgentMessage: ({ agent_id, message, attachments, forward }) => {
			// The forwarded item, live, as the daemon looks it up.
			let content: AgentMessageContent = { kind: 'text', text: message };
			if (forward?.kind === 'task') {
				const { title } = task(forward.workspace_id, forward.id);
				content = { kind: 'forward', note: message.trim(), item: { ...forward, title } };
			} else if (forward) {
				const { title, document_type } = document(forward.workspace_id, forward.id);
				const kind = document_type === 'journal' ? 'journal' : 'document';
				content = { kind: 'forward', note: message.trim(), item: { ...forward, kind, title } };
			}
			// Like the daemon's trigger: the agent's reply leaves it unread.
			agent(agent_id).agent_unread = true;
			const now = nowSeconds();
			const thread_id = nextId(state.threads);
			state.threads.push({
				id: thread_id,
				created_at: now,
				prompt_id: 1,
				system_prompt: '',
				origin: { kind: 'root' },
				remote: null
			});
			state.costEvents.push(costEvent(nextId(state.costEvents), thread_id, now, 6_000, 120));
			const user = agentMessage(nextId(state.agentMessages), agent_id, now, 'user', content, {
				attachment_count: attachments?.length ?? 0
			});
			state.agentMessages.push(user);
			for (const { media_type, data } of attachments ?? [])
				state.agentAttachments.push({
					id: nextId(state.agentAttachments),
					message: { kind: 'agent', agent_message_id: user.id },
					media_type,
					data
				});
			const reply = agentMessage(
				nextId(state.agentMessages),
				agent_id,
				now,
				'agent',
				`On it: ${message}`
			);
			state.agentMessages.push(reply);
			return null;
		},
		// The mock's turns end as they start, so there is never one to stop.
		stopAgentTurn: () => null,
		// The pick is recorded on the choice, and the agent's canned reply
		// follows; no message of the user's is posted.
		respondToAgentChoice: ({ message_id, option }) => {
			const m = agentMessageById(message_id);
			const choice = m.message_content;
			if (choice.kind !== 'choice')
				throw new ProtocolError('validationFailed', 'That message isn’t a choice.');
			if (choice.selected !== null)
				throw new ProtocolError('validationFailed', 'That choice has already been answered.');
			if (option >= choice.options.length)
				throw new ProtocolError('validationFailed', 'That choice has no such option.');
			choice.selected = option;
			agent(m.agent_id).agent_unread = true;
			const reply = agentMessage(
				nextId(state.agentMessages),
				m.agent_id,
				nowSeconds(),
				'agent',
				`Going with ${choice.options[option]}.`
			);
			state.agentMessages.push(reply);
			return null;
		},
		listAgentCues: ({ agent_id }) => {
			agent(agent_id);
			return state.agentCues.filter((c) => c.agent_id === agent_id).map((c) => ({ ...c }));
		},
		// What the agent sees, its own, then its team's, then the global ones; the global ones alone
		// without an agent.
		listMemories: ({ agent_id }) => {
			const seer = agent_id == null ? null : agent(agent_id);
			const sees = ({ scope }: Memory) =>
				scope.kind === 'global' ||
				(scope.kind === 'agent' && scope.agent_id === seer?.id) ||
				(scope.kind === 'team' && scope.team_id === seer?.team_id);
			const rank = { agent: 0, team: 1, global: 2 };
			return state.memories
				.filter(sees)
				.toSorted((a, b) => rank[a.scope.kind] - rank[b.scope.kind] || a.id - b.id)
				.map((m) => structuredClone(m));
		},
		forgetMemory: ({ memory_id }) => {
			const memory = state.memories.find((m) => m.id === memory_id) ?? notFound('memory');
			state.memories = state.memories.filter((m) => m.id !== memory_id);
			return { ...memory };
		},
		removeAgentCue: ({ cue_id }) => {
			if (!state.agentCues.some((c) => c.id === cue_id)) notFound('intelligence agent cue');
			state.agentCues = state.agentCues.filter((c) => c.id !== cue_id);
			return null;
		},
		listActors: () => state.actors.filter((a) => !a.deleted).map((a) => structuredClone(a)),
		// Both actors draw on one account, which their client names.
		listActorUsage: () => [
			{
				actor_ids: [1, 2],
				pool: { id: 'claude:me@example.com', name: 'me@example.com' },
				plan: [
					{ label: '5-hour limit', used: 0.84, resets_at: nowSeconds() + 2 * 3600 },
					{ label: 'Weekly limit', used: 0.18, resets_at: nowSeconds() + 4 * 86_400 }
				]
			}
		],
		listActorSessions: ({ actor_id }) =>
			state.actorSessions
				.filter(
					(t) =>
						t.status !== 'resolved' &&
						t.subagent === null &&
						(actor_id == null || t.actor_id === actor_id)
				)
				.sort((a, b) => b.status_changed_at - a.status_changed_at || b.id - a.id)
				.map((t) => ({ ...t })),
		getActorSession: ({ id }) => structuredClone(actorSession(id)),
		listActorSubagents: ({ session_id }) => {
			actorSession(session_id);
			return state.actorSessions
				.filter((t) => t.subagent?.parent_session_id === session_id)
				.sort((a, b) => a.started_at - b.started_at || a.id - b.id)
				.map((t) => structuredClone(t));
		},
		listActorSessionFamily: ({ session_id }) => {
			let top = actorSession(session_id);
			while (top.subagent) top = actorSession(top.subagent.parent_session_id);
			const below = (id: number): ActorSession[] =>
				state.actorSessions
					.filter((t) => t.subagent?.parent_session_id === id)
					.sort((a, b) => a.started_at - b.started_at || a.id - b.id)
					.flatMap((t) => [t, ...below(t.id)]);
			return [top, ...below(top.id)].map((t) => structuredClone(t));
		},
		listUnansweredSubagentRequests: ({ session_id }) => {
			actorSession(session_id);
			const going = subagentsBelow(session_id)
				.filter((t) => t.status === 'working' || t.status === 'needsAttention')
				.map((t) => t.id);
			return state.actorSessionMessages
				.filter(
					(m) =>
						going.includes(m.session_id) &&
						m.content.sender === 'actor' &&
						['question', 'permission', 'plan'].includes(m.content.kind) &&
						!m.answered
				)
				.sort((a, b) => a.id - b.id)
				.map((m) => structuredClone(m));
		},
		// Every word in the title, or in one text message; a title match first.
		searchActorSessions: ({ query }) => {
			const words = searchWords(query);
			if (words.length === 0) return [];
			const matches = (text: string) => words.every(starts(text));
			const hits: ActorSessionSearchHit[] = [];
			for (const t of state.actorSessions) {
				if (t.subagent) continue;
				const hit = {
					id: t.id,
					actor_id: t.actor_id,
					title: t.title,
					status_changed_at: t.status_changed_at
				};
				if (matches(t.title)) {
					hits.push({ ...hit, excerpt: null });
					continue;
				}
				const said = state.actorSessionMessages.find(
					(m) => m.session_id === t.id && m.content.kind === 'text' && matches(m.content.text)
				);
				if (said?.content.kind === 'text') hits.push({ ...hit, excerpt: said.content.text });
			}
			return hits.sort((a, b) => Number(a.excerpt !== null) - Number(b.excerpt !== null));
		},
		markActorSessionRead: ({ session_id }) => {
			const session = actorSession(session_id);
			session.unread = false;
			session.read_at = nowSeconds();
			return { ...session };
		},
		listActorSessionMessages: ({ session_id }) => {
			actorSession(session_id);
			return state.actorSessionMessages
				.filter((m) => m.session_id === session_id)
				.map((m) => structuredClone(m));
		},
		listActorSessionMessageAttachments: ({ message_id }) =>
			state.actorSessionAttachments
				.filter(
					(a) =>
						a.message.kind === 'actorSession' && a.message.actor_session_message_id === message_id
				)
				.map((a) => ({ ...a })),
		startActorSession: ({ actor_id, prompt, title, mode, model, effort, attachments }) => {
			const a = state.actors.find((x) => x.id === actor_id && !x.deleted) ?? notFound('actor');
			if (!clientServes(a.client_id))
				throw new ProtocolError('validationFailed', `${a.title} is offline.`);
			const text = prompt.trim();
			if (!text) throw new ProtocolError('validationFailed', 'Say what to do.', 'prompt');
			const now = nowSeconds();
			const session: ActorSession = {
				id: nextId(state.actorSessions),
				actor_id,
				title: title?.trim().split('\n')[0] || text.split('\n')[0],
				mode: mode ?? null,
				model: model ?? null,
				effort: effort ?? null,
				started_at: now,
				status_changed_at: now,
				status: 'working',
				unread: false,
				read_at: null,
				context: null,
				follower: null,
				subagent: null
			};
			state.actorSessions.push(session);
			const message: ActorSessionMessage = {
				id: nextId(state.actorSessionMessages),
				session_id: session.id,
				sent_at: now,
				content: { sender: 'user', kind: 'text', text },
				answered: false,
				attachment_count: attachments?.length ?? 0
			};
			state.actorSessionMessages.push(message);
			attachActorImages(message.id, attachments);
			return { ...session };
		},
		sendActorSessionMessage: ({ session_id, content, attachments }) => {
			// A message to a session that is done or failed picks it back up.
			const session = actorSession(session_id);
			const now = nowSeconds();
			if (session.subagent && !('request_id' in content))
				throw new ProtocolError(
					'validationFailed',
					'A subagent takes only responses to what it asked.'
				);
			if ('request_id' in content) {
				const request = state.actorSessionMessages.find((m) => m.id === content.request_id);
				if (request) request.answered = true;
			} else {
				// Words pass over every request still open in the session.
				for (const m of state.actorSessionMessages) {
					const { sender, kind } = m.content;
					if (
						m.session_id === session_id &&
						sender === 'actor' &&
						['question', 'permission', 'plan'].includes(kind)
					)
						m.answered = true;
				}
			}
			const message: ActorSessionMessage = {
				id: nextId(state.actorSessionMessages),
				session_id,
				sent_at: now,
				content: { sender: 'user', ...content } as ActorSessionMessageContent,
				answered: false,
				attachment_count: attachments?.length ?? 0
			};
			state.actorSessionMessages.push(message);
			attachActorImages(message.id, attachments);
			if (session.status !== 'working') {
				session.status = 'working';
				session.status_changed_at = now;
			}
			return structuredClone(message);
		},
		stopActorSession: ({ session_id }) => {
			const session = startedSession(session_id);
			if (session.status !== 'idle' && session.status !== 'resolved') {
				session.status = 'idle';
				session.status_changed_at = nowSeconds();
			}
			return { ...session };
		},
		resolveActorSession: ({ session_id }) => {
			const session = startedSession(session_id);
			if (session.status !== 'resolved') {
				session.status = 'resolved';
				session.status_changed_at = nowSeconds();
			}
			return { ...session };
		},
		setActorSessionMode: ({ session_id, mode }) => {
			const session = startedSession(session_id);
			session.mode = mode;
			return { ...session };
		},
		listActivities: ({ limit }) => {
			const running = state.activities
				.filter((a) => a.ended_at === null)
				.sort((a, b) => b.started_at - a.started_at);
			const ended = state.activities
				.filter((a) => a.ended_at !== null)
				.sort((a, b) => b.ended_at! - a.ended_at!)
				.slice(0, limit);
			return [...running, ...ended].map((a) => structuredClone(a));
		},
		listAgentRuns: ({ agent_id, limit }) => {
			agent(agent_id);
			return state.agentRuns
				.filter((r) => r.agent_id === agent_id)
				.sort((a, b) => b.started_at - a.started_at)
				.slice(0, limit)
				.map((r) => ({ ...r }));
		}
	};

	const send = (<K extends RequestType>(type: K, ...args: ArgsOf<K>): Promise<Replies[K]> => {
		try {
			const handler = handlers[type] as (args: Requests[K] | undefined) => Replies[K];
			return Promise.resolve(handler(args[0] as Requests[K] | undefined));
		} catch (error) {
			return Promise.reject(error);
		}
	}) as Send;

	// Rebuilds the frame the daemon would send, error replies included.
	const sendRaw = async ({ type, args }: RawRequest): Promise<ReplyFrame> => {
		const handler = (handlers as Record<string, ((args: unknown) => unknown) | undefined>)[type];
		if (!handler)
			return { error: { code: 'badRequest', message: `Unknown request type ${type}.` } };
		try {
			return { ok: { type, data: handler(args) } };
		} catch (error) {
			const e = toProtocolError(error);
			const code = e.code === 'disconnected' ? 'internalError' : e.code;
			return { error: { code, message: e.message, ...(e.field ? { field: e.field } : {}) } };
		}
	};

	return { send, sendRaw, state };
}

/** Refuses an export or import: mock mode has no database file to move. */
function noTransfers(): never {
	throw new ProtocolError('badRequest', 'Mock mode has no data to export or import.');
}
