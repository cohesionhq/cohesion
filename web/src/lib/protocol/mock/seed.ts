// The data a mock daemon starts from, and the builders of its rows that the
// handlers use too.

import type {
	Actor,
	ActorMessageContent,
	ActorSessionMessage,
	CalendarEvent,
	IntelligenceAgentColor,
	ActorSessionMessageContent,
	AgentChoice,
	AgentCodeExecution,
	AgentMemoryChange,
	AgentMessageContent,
	AgentRunPart,
	Article,
	CatalogModel,
	Component,
	ContactMethodKind,
	ContextRebuildReason,
	Dialect,
	Document,
	EndpointModel,
	ExtensionManifest,
	ExtensionPermissionKind,
	Feed,
	FormulaUpdateMode,
	HookKind,
	IntelligenceAgentMessage,
	IntelligenceCostEvent,
	IntelligenceEndpointPreset,
	IntelligenceSpend,
	IntelligenceThreadMessage,
	LongCache,
	Photo,
	PropertyDataType,
	PropertyValue,
	RegistryResult,
	SecurityEvent,
	SecurityEventKind,
	SpendPeriod,
	Task,
	TaskStatus,
	ThreadItem,
	Version,
	VersionAuthor,
	VersionChange
} from '../types';
import type { MockConnector, MockRegistry, MockSource, MockState, TranscriptItem } from './state';
import { DAY, MOCK_VOCABULARY, dayIn, localMidnight, nowSeconds, ymd } from './support';
import { computeFormulas, folderDocuments, storeValue, typedProperty } from './tableRules';

/** A stored message the way the daemon writes one. */
export function message(
	id: number,
	thread_id: number,
	created_at: number,
	item: TranscriptItem
): IntelligenceThreadMessage {
	return { id, thread_id, created_at, item: storedItem(item) };
}

/** A transcript item as a thread message holds it. */
export function storedItem(item: TranscriptItem): ThreadItem {
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
export function securityEvent(
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
export const SPEND_PERIODS: [SpendPeriod, number][] = [
	['hour', 3600],
	['day', DAY],
	['month', 30 * DAY]
];

/** What the cost events of each period add up to, against its limit, as the daemon sums them. */
export function spend(state: MockState): IntelligenceSpend[] {
	const now = nowSeconds();
	return SPEND_PERIODS.map(([period, seconds]) => {
		const spent = state.costEvents
			.filter((e) => e.created_at > now - seconds)
			.reduce((sum, e) => sum + e.total_cost, 0);
		const limit = state.spendLimits.find((l) => l.period === period)?.amount ?? null;
		return { period, limit, spent, reached: limit !== null && spent >= limit };
	});
}

export function costEvent(
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
export const PRESET_DEFAULTS: Record<
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
export function fromCatalog(id: number, endpoint_id: number, catalog: CatalogModel): EndpointModel {
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

/** A plausible extension script whose `manifest()` returns `manifest`. */
export function extensionScript(manifest: ExtensionManifest): string {
	return `function manifest() {
  return ${JSON.stringify(manifest, null, 2).replaceAll('\n', '\n  ')};
}

export function onLaunch() {
  log.info("${manifest.name} started");
}
`;
}

export function extensionManifest(
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
export function seedSources(): Map<string, MockSource> {
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

export function seedRegistries(sources: Map<string, MockSource>): MockRegistry[] {
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

export const NOTION_ICON =
	'data:image/svg+xml,' +
	encodeURIComponent(
		'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><rect width="100" height="100" rx="20" fill="#191919"/><text x="50" y="68" font-size="56" font-weight="700" text-anchor="middle" fill="#fff" font-family="Georgia, serif">N</text></svg>'
	);

/**
 * The Launch plan page as its connector draws it: every kind of block, at
 * each heading level, with a checkbox and a link inside overviews.
 */
export function launchPlanView(): Component {
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
export function seedConnectors(now: number): MockConnector[] {
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

export function seed(): MockState {
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
			folder_id: null,
			pinned: false
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
		folder_id: filed[id] ?? null,
		pinned: id === 5 || id === 8
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
			{ id: 1, title: 'Cohesion', emoji: '📱', sort_order: 0, deleted: false },
			{ id: 2, title: 'Church', emoji: '⛪', sort_order: 1, deleted: false },
			{ id: 3, title: 'Containers', emoji: '📦', sort_order: 2, deleted: false }
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
				'Each Sunday needs a producer, a run-through lead, sound, lighting, Pro-Presenter and camera. Planning Center sends the rota; tasks follow it.\n\n| Sunday | Producer | Run-through lead | Sound | Lighting | Pro-Presenter | Camera | Notes |\n| --- | --- | --- | --- | --- | --- | --- | --- |\n| 16 November | Maria Lopez | Daniel Reyes | Sam Whitaker | Priya Nair | Ethan Brooks | Hannah Cole | Baptism after the second service; camera covers the font |\n| 23 November | Daniel Reyes | Maria Lopez | Hannah Cole | Sam Whitaker | Priya Nair | Ethan Brooks | Guest band, so sound check moves to 7:30 |\n| 30 November | Priya Nair | Ethan Brooks | Sam Whitaker | Hannah Cole | Maria Lopez | Daniel Reyes | First Sunday of Advent |\n'
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
				'# Launch plan\n\nWhat has to land before the beta, as cards. A link in a sentence, such as [the search notes](cohesion://document/1/5), stays a link.\n\n[Install extension from URL](cohesion://task/1/4)\n[Cohesion-bench](cohesion://task/1/2)\n\n## Who is on it\n\n[Secretary](cohesion://agent/2)\n[the install session](cohesion://actor-session/1)\n[Mira Okafor](cohesion://contact/1/1)\n[Tomás Reyes](cohesion://contact/1/2)\n\n## Reading\n\n[Notion page](cohesion://resource/1/1)\n[Article](cohesion://article/1)\n\n## Dropped\n\n[Old migration task](cohesion://task/1/999)\n'
			)
		],
		documentFolders: [
			{
				id: 1,
				workspace_id: 1,
				parent_id: null,
				title: 'Design',
				sort_order: 1,
				deleted: false,
				representation: 'list',
				pinned: true
			},
			{
				id: 2,
				workspace_id: 1,
				parent_id: null,
				title: 'Research',
				sort_order: 2,
				deleted: false,
				representation: 'list',
				pinned: false
			},
			{
				id: 3,
				workspace_id: 1,
				parent_id: 2,
				title: 'Search',
				sort_order: 1,
				deleted: false,
				representation: 'list',
				pinned: true
			}
		],
		folders: [
			{
				id: 1,
				workspace_id: 1,
				title: 'Core',
				color: 'blue',
				symbol: 'code',
				pinned: true,
				sort_order: 1,
				deleted: false
			},
			{
				id: 2,
				workspace_id: 1,
				title: 'WebUI',
				color: 'purple',
				symbol: 'globe',
				pinned: true,
				sort_order: 2,
				deleted: false
			},
			{
				id: 3,
				workspace_id: 1,
				title: 'Plus',
				color: 'orange',
				symbol: null,
				pinned: false,
				sort_order: 3,
				deleted: false
			},
			{
				id: 4,
				workspace_id: 1,
				title: 'Cloud',
				color: null,
				symbol: 'lock',
				pinned: false,
				sort_order: 4,
				deleted: false
			},
			{
				id: 5,
				workspace_id: 1,
				title: 'JavaScript',
				color: 'yellow',
				symbol: null,
				pinned: false,
				sort_order: 5,
				deleted: false
			},
			{
				id: 6,
				workspace_id: 1,
				title: 'WebUI Harnesses',
				color: 'teal',
				symbol: 'tools',
				pinned: false,
				sort_order: 6,
				deleted: false
			},
			{
				id: 7,
				workspace_id: 2,
				title: 'PCO',
				color: null,
				symbol: null,
				pinned: false,
				sort_order: 1,
				deleted: false
			}
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
				'Needs design: recurrence rules, what completing a repeat does, and how extensions like Todoist map.\n\n[Area: Tasks](cohesion://document/1/5)\n\nRelated: [Subagent sessions](cohesion://task/1/10).',
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
		experiments: [],
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
		...seedCalendars(now),
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
		savedTaskViews: [],
		folderProperties: [],
		documentValues: []
	};
}

/** A contact's photo: a 64-pixel PNG of a head and shoulders. */
export const CONTACT_PHOTO: Photo = {
	kind: 'image',
	photo_media_type: 'image/png',
	photo_data:
		'iVBORw0KGgoAAAANSUhEUgAAAEAAAABACAIAAAAlC+aJAAACAklEQVR42tXPWVLCUBCF4bM6F+FyXE6QSUAIMgkBAoEwyCguwGVIeTF1DRBC91XSVd9rV/8HlvshGqz+TjSk+jvRkOrtRMNj7100pLvvoskfkHG2oiHrbEVDtrMVDbnORjTk2hvRkG9vRMPT61q0Pxnw6VjnmB9QaK0NikjXGfyIYmttRMx0nZG/+wErPkL9zwbua5SaKyZyvcL8Ln/Ac2PJwaxXOAEoN5YcRgZwAlCuL8mM1B82UBtQqS/IDA4gN8CuLcgMDiA3yB9QfXkjMziA3CB/wEv1jczgAHIDatU5mcEB5AbU7DmZyQHUBtTtOYeRek4AGvacw8gATgAalRkTt573Hc3KjIk5gPkdzfKM6e7+gVy/v2V+R6s8Jdu/D9DqA+QGvD5PCfTfhBnnzgklMFgfZ8bF26sHtEuTq1ws4LuqB53SJL5/qFfiJ6FTnMTxb+m/ZsQIg1P047jJgDhhcAr+RTepP2y41IZuwY92w3olOg+9p3GEm9crEYVIfn30BvTz43MSNeBcJNz8+KRE1SsnO/cDRiclcsCJTgxyo2MJrFeOU+UPGGa9kMTWK6FaeFkvJOEDQrXwMp4u4fWHDVowRpmhTsQAPRjj9FAnYoAeDHH1oQ3wHwcBQQOCZvkDJqlBQNCAoBnTlKsIqldUNqaWq8gb8J2NmeUq4gao7C92itymzsyHpAAAAABJRU5ErkJggg=='
};

/**
 * Cohesion's own store with two calendars, and an account's store (synced
 * an hour ago, keeping colors of its own) with a calendar and a read-only
 * one of holidays; the first workspace shows all but the holidays. Events through this
 * week: overlapping ones today, an all-day one over three days, one with
 * invitees, and a holiday.
 */
export function seedCalendars(
	now: number
): Pick<MockState, 'calendarStores' | 'calendars' | 'calendarEvents' | 'calendarPerspectives'> {
	const zone = Intl.DateTimeFormat().resolvedOptions().timeZone;
	const base = {
		deleted: false,
		location: '',
		description: '',
		invitees: [],
		selected_color: null,
		declared_color: null,
		remote_identifier: null,
		remote_modified: null,
		pending_push: false,
		revision: 0,
		created_at: now - DAY,
		modified: now - DAY
	};
	/** A timed event `day` days from today, from `from` to `to` in hours. */
	const timed = (
		id: number,
		calendar_id: number,
		title: string,
		day: number,
		from: number,
		to: number,
		more: Partial<CalendarEvent> = {}
	): CalendarEvent => ({
		...base,
		id,
		calendar_id,
		title,
		timing: {
			kind: 'timed',
			starts_at: localMidnight(-day) + from * 3_600,
			ends_at: localMidnight(-day) + to * 3_600,
			time_zone: zone
		},
		...more
	});
	const allDay = (
		id: number,
		calendar_id: number,
		title: string,
		from: number,
		to: number
	): CalendarEvent => ({
		...base,
		id,
		calendar_id,
		title,
		timing: { kind: 'allDay', start_date: dayIn(from), end_date: dayIn(to) }
	});
	const calendar = (
		id: number,
		store_id: number,
		title: string,
		color: IntelligenceAgentColor,
		read_only = false
	) => ({
		id,
		store_id,
		deleted: false,
		title,
		selected_color: color,
		declared_color: store_id === 2 ? color : null,
		color_pending_push: false,
		remote_identifier: store_id === 2 ? `cal-${id}` : null,
		read_only,
		created_at: now - 7 * DAY,
		modified: now - 7 * DAY
	});
	return {
		calendarStores: [
			{
				id: 1,
				title: 'Cohesion',
				named_by_user: false,
				backend: { kind: 'local' },
				color_support: 'none',
				last_synced_at: null,
				sync_failure: null,
				created_at: now - 7 * DAY
			},
			{
				id: 2,
				title: 'sam@example.com',
				named_by_user: false,
				backend: { kind: 'account', account_id: 1 },
				color_support: 'declared',
				last_synced_at: now - 3_600,
				sync_failure: null,
				created_at: now - 7 * DAY
			}
		],
		calendars: [
			calendar(1, 1, 'Home', 'teal'),
			calendar(2, 1, 'Fitness', 'orange'),
			calendar(3, 2, 'Work', 'blue'),
			calendar(4, 2, 'Holidays', 'green', true)
		],
		calendarPerspectives: { 1: [1, 2, 3] },
		calendarEvents: [
			timed(1, 1, 'Dentist', 0, 10, 11, { location: 'High Street' }),
			timed(2, 1, 'Piano lesson', 0, 17, 18),
			timed(3, 3, 'Design review', 0, 10.5, 12, {
				location: 'Room 4',
				description: 'Walk through the calendar views before they ship.',
				invitees: [
					{ email: 'priya@example.com', name: 'Priya Natarajan', response: 'accepted' },
					{ email: 'tom@example.com', name: null, response: 'tentative' },
					{ email: 'lee@example.com', name: 'Lee Okafor', response: 'needsAction' }
				]
			}),
			timed(4, 3, 'Standup', 1, 9, 9.25),
			timed(5, 3, 'Standup', 2, 9, 9.25),
			timed(6, 2, 'Run', -1, 7, 8),
			timed(7, 2, 'Climbing', 2, 18.5, 20.5, { selected_color: 'red' }),
			timed(8, 3, 'Planning', -2, 14, 15.5),
			allDay(9, 1, 'Visiting Mum', 1, 3),
			allDay(10, 4, 'Bank holiday', 4, 4),
			timed(11, 3, 'Lunch with Priya', 1, 12.5, 13.5, { location: 'Corner café' })
		]
	};
}

/** Three contacts in the first workspace, the first with a photo. */
export function seedContacts(now: number): Pick<MockState, 'contacts' | 'contactMethods'> {
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

// ---- Tables -----------------------------------------------------------------
// A document folder with properties is a table, and each document in it a
// row. The daemon filters and sorts rows in SQL and runs formulas in QuickJS;
// the mock does both in JS, by the same rules (`document_value.rs`,
// `document_formula.rs`), reading only a small part of the language for a
// formula (`mockFormula.ts`).

/**
 * Two table folders in the first workspace: a reading list holding a value
 * of every type and a formula that fails on the row with no rating, and a
 * renovation budget whose total is a formula computed continuously. The
 * first row has notes, and a document links to the reading list and to one
 * of its rows.
 */
export function seedTables(state: MockState) {
	type Cell = string | number | boolean | null;
	const add = (
		title: string,
		columns: [string, PropertyDataType, string?, FormulaUpdateMode?][],
		rows: [string, ...Cell[]][]
	): number => {
		const folder_id = state.documentFolders.reduce((max, f) => Math.max(max, f.id), 0) + 1;
		state.documentFolders.push({
			id: folder_id,
			workspace_id: 1,
			parent_id: null,
			title,
			sort_order: folder_id,
			deleted: false,
			representation: 'table',
			pinned: false
		});
		const first = state.folderProperties.length + 1;
		const properties = columns.map(([name, data_type, formula, update_mode], i) =>
			typedProperty(
				{
					id: first + i,
					folder_id,
					name,
					sort_order: i + 1,
					formula: formula
						? { source: formula, update_mode: update_mode ?? 'whenChanged', error: null }
						: null
				},
				data_type
			)
		);
		state.folderProperties.push(...properties);
		const inputs = properties.filter((p) => p.formula === null);
		rows.forEach(([rowTitle, ...cells], i) => {
			const id = state.documents.reduce((max, d) => Math.max(max, d.id), 0) + 1;
			state.documents.push({
				id,
				workspace_id: 1,
				document_type: 'normal',
				title: rowTitle,
				contents: '',
				modified: nowSeconds() - (10 + i) * DAY,
				journal_date: null,
				deleted: false,
				folder_id,
				pinned: false
			});
			cells.forEach((cell, i) => {
				if (cell === null) return;
				const p = inputs[i];
				storeValue(state, id, p.id, { type: p.data_type, value: cell } as PropertyValue);
			});
		});
		computeFormulas(state, folder_id);
		return folder_id;
	};
	const reading = add(
		'Reading list',
		[
			['Author', 'text'],
			['Pages', 'integer'],
			['Rating', 'decimal'],
			['Price', 'currency'],
			['Read', 'checkbox'],
			['Finished', 'date'],
			['Stars', 'text', "(row) => row.Rating.toFixed(1) + ' stars'"]
		],
		[
			['Dune', 'Frank Herbert', 412, 4.5, 9.99, true, '2026-09-14'],
			['The Left Hand of Darkness', 'Ursula K. Le Guin', 304, 4.8, 12.5, true, '2026-10-02'],
			['Piranesi', 'Susanna Clarke', 272, 4.2, 14, false, null],
			['Project Hail Mary', 'Andy Weir', 496, null, 16.99, false, null],
			['The Dispossessed', 'Ursula K. Le Guin', 387, 4.6, 11.25, true, '2026-08-21'],
			['Klara and the Sun', 'Kazuo Ishiguro', 303, 3.9, null, null, null]
		]
	);
	add(
		'Kitchen renovation',
		[
			['Supplier', 'text'],
			['Quantity', 'integer'],
			['Unit price', 'currency'],
			['Total', 'currency', '(row) => row.Quantity * row["Unit price"]', 'continuously'],
			['Ordered', 'checkbox'],
			['Delivery', 'date']
		],
		[
			['Oak worktop', 'Timber & Co', 2, 340, true, '2026-11-18'],
			['Mixer tap', 'Bathstore', 1, 129.99, true, '2026-11-12'],
			['Cabinet handles', 'Handle Shop', 14, 6.5, false, null],
			['Wall tiles (m²)', 'Tile Depot', 9, 32, false, '2026-11-25'],
			['Paint', null, 3, 24.75, null, null]
		]
	);
	// A choice on the reading list: where each book is up to.
	const status = typedProperty(
		{
			id: state.folderProperties.length + 1,
			folder_id: reading,
			name: 'Status',
			sort_order: 8,
			formula: null
		},
		'choice',
		(
			[
				['To read', 'blue'],
				['Reading', 'orange'],
				['Finished', 'green']
			] as const
		).map(([name, color], i) => ({
			id: i + 1,
			property_id: state.folderProperties.length + 1,
			name,
			color,
			sort_order: i
		}))
	);
	state.folderProperties.push(status);
	folderDocuments(state, reading).forEach((doc, i) => {
		// Finished, finished, reading, to read, finished; the last has none.
		const option = [3, 3, 2, 1, 3][i];
		if (option) storeValue(state, doc.id, status.id, { type: 'choice', value: option });
	});
	const [dune, , piranesi] = folderDocuments(state, reading);
	dune.contents =
		'# Notes\n\nRead it on the train over two weeks. The appendices are worth it.\n\n- Lend to Mira next.\n';
	// A document showing the reading list and one of its rows as cards, which
	// their backlinks list.
	state.documents.push({
		id: state.documents.reduce((max, d) => Math.max(max, d.id), 0) + 1,
		workspace_id: 1,
		document_type: 'normal',
		title: 'Winter reading',
		contents: `Books for the long evenings.\n\n[Reading list](cohesion://document-folder/1/${reading})\n\nNext up:\n\n[Piranesi](cohesion://document/1/${piranesi.id})\n`,
		modified: nowSeconds() - 2 * DAY,
		journal_date: null,
		deleted: false,
		folder_id: null,
		pinned: false
	});
}

/** Six articles across the feeds: one read and one whose page couldn't be fetched among them. */
export function seedArticles(now: number): Article[] {
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
export function feedItem(
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
export function seedFeeds(now: number): Feed[] {
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
export function seedConnectorData(
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
export const rebuildBecause: Record<ContextRebuildReason | 'none', string> = {
	cacheExpired: ' because provider cache expired',
	contextLimit: ' because context reached its size limit',
	memoryCapacityChanged: ' because memory capacity changed',
	modelChanged: ' because the model changed',
	noThread: ' because there was no live thread',
	turnFailed: ' because the last turn failed',
	none: ''
};

/**
 * A message in one line (what follows its verb, for one that has one), as the daemon's `IntelligenceAgentMessage::EXCERPT_SQL`
 * words it, so mock mode can stand in for the daemon.
 */
export function agentExcerpt(content: AgentMessageContent, attachment_count: number): string {
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
			case 'skill':
				return `Skill “${content.name}”${content.note.trim() ? `: ${content.note}` : ''}`;
			case 'usedSkill':
				return `Using skill “${content.name}”`;
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
			case 'agenda':
				return content.items.join('; ');
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
export function agentRunPart(
	content: AgentMessageContent,
	run_id: number | null
): AgentRunPart | null {
	if (run_id === null) return null;
	switch (content.kind) {
		case 'woke':
		case 'askedByAgent':
			return 'trigger';
		case 'codeExecution':
		case 'documentation':
		case 'memory':
		case 'note':
		case 'agenda':
		case 'askedAgent':
		case 'usedSkill':
			return 'step';
		default:
			return 'surfaced';
	}
}

/** What the agent did, for a line of its bookkeeping, as `EXCERPT_VERB_SQL` words it. */
export function agentExcerptVerb(content: AgentMessageContent): string | null {
	switch (content.kind) {
		case 'memory':
			return content.change === 'forgotten' ? 'Forgot' : 'Remembered';
		case 'note':
			return 'Noted';
		case 'agenda':
			return { appended: 'Added to the agenda', finished: 'Finished', reset: 'Cleared the agenda' }[
				content.change
			];
		case 'woke':
			return 'Woke';
		default:
			return null;
	}
}

export function agentMessage(
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
		switched_workspace_id: null,
		switched_workspace_title: null,
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
export function reactionExcerpt(
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
export function seedActors(
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
export const WEEK_CHART =
	'iVBORw0KGgoAAAANSUhEUgAAAPAAAACMCAIAAADN17N/AAAB5UlEQVR42u3dwQ1AERRFQR3qRE3WCsRSFYRnklPAjTfLn/zU+5TClDyBgJaAloCWgBbQEtAS0BLQEtACWgJaAloCWgJaQEtAS0BLQEtABy+XsSmgBTTQAhpoAQ000EALaKAFNNACGmgBDTTQQAtooAU00AIaaKCBBhpooAU00AIaaAENNNBAC2igBfR9G4AGGmgBDbSABlpAAw000AIaaAENtIAGWkADDTTQAhpoAQ20gAZaQIcH7TtgoIEGGmiH9A5AAw000EADDTTQQAMNNNAO6R2ABhpooIEGGmiggQYaaKAd0jsADTTQQAMNNNBAAw000EA7pHcAGmiggQYaaKB/OqQNQMMENNA2AA20DUA7pA1AwwQ00DABDbQNQANtA9AOaQPQMAENNExAA20D0EDbALRD2nAAtEe0AWiHtAFoG2wA2gaggbYBaIe0AWiHtAFoG2wA2gagPaINQDukDUA7pA1A22AD0DYA7RFtANohbQDaIW0A2gYbgLYBaI9oA9AOaQPQDmkD0DbYALQNQHtEG4B2SBuAdkgbgLbBBqBtANoj2gC0Q9oAtA02AG0D0EDbALRHtAFoh7QBaBtsANoGoIG2AWiPaMNLoGtt0lUF/5OsBLSAloCWgJaAloAW0BLQ0qkWqfkkEnJUAs8AAAAASUVORK5CYII=';

/** The steps the seeded Core Engineer set out, its first one finished. */
export const AGENDA = [
	'Add the agenda table and its commands',
	'Give every thread the agenda tools, with descriptions that say the agenda is a record the user checks in on, not a queue',
	'Show the agenda beside the conversation, keeping long steps from wrapping down the column',
	'Run the checks',
	'Check it in a browser at desktop width, in light and dark mode, and take a screenshot of each'
];

export function seedAgents(
	now: number
): Pick<
	MockState,
	| 'agents'
	| 'agentTeams'
	| 'agentMessages'
	| 'agentAttachments'
	| 'agentCues'
	| 'agentAgenda'
	| 'agentRuns'
	| 'memories'
	| 'agentSkills'
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
				workspace_id: null,
				endpoint_model_id: null,
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
				workspace_id: null,
				endpoint_model_id: null,
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
				workspace_id: null,
				endpoint_model_id: null,
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
				workspace_id: null,
				endpoint_model_id: null,
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
			// The agent sets out its agenda, then finishes the first item.
			agentMessage(59, 1, now - 61, 'agent', {
				kind: 'agenda',
				change: 'appended',
				items: AGENDA
			}),
			agentMessage(60, 1, now - 61, 'user', {
				kind: 'skill',
				name: 'Standup',
				prompt: 'Say what you finished yesterday, what you’re on today and what’s blocking you.',
				note: 'Keep it to Core.'
			}),
			agentMessage(
				61,
				1,
				now - 61,
				'agent',
				'Finished the backlinks migration. Today: **Repeating tasks**. Nothing blocking.'
			),
			agentMessage(62, 1, now - 61, 'agent', { kind: 'usedSkill', name: 'Release notes' }),
			agentMessage(
				63,
				1,
				now - 61,
				'agent',
				'- **Documents**: tables, with formulas.\n- **Agents**: memories per team.'
			),
			agentMessage(64, 1, now - 20, 'agent', {
				kind: 'agenda',
				change: 'finished',
				items: [AGENDA[0]]
			}),
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
		agentAgenda: AGENDA.map((text, index) => ({
			id: index + 1,
			agent_id: 1,
			position: index + 1,
			text,
			added_at: now - 61,
			finished_at: index === 0 ? now - 20 : null
		})),
		agentSkills: [
			{
				id: 1,
				scope: { kind: 'team', team_id: 1 },
				name: 'Release notes',
				description: 'When the user asks what shipped.',
				prompt:
					'List what merged to master since the last tag, grouped by area, one plain line each. Leave out refactors and chores.',
				agent_can_use: true,
				created_at: now - 4 * DAY
			},
			{
				id: 2,
				scope: { kind: 'agent', agent_id: 1 },
				name: 'Triage',
				description: 'When a bug report comes in.',
				prompt:
					'Find where the bug is, say how sure you are, and file a task in Core with the steps to reproduce.',
				agent_can_use: true,
				created_at: now - 3 * DAY
			},
			{
				id: 3,
				scope: { kind: 'agent', agent_id: 1 },
				name: 'Standup',
				description: '',
				prompt: 'Say what you finished yesterday, what you’re on today and what’s blocking you.',
				agent_can_use: false,
				created_at: now - 2 * DAY
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

/**
 * Rewind's history of workspace 1's first document and first task, by each kind
 * of author, and a deleted item of each kind to bring back.
 */
export function seedRewind(state: MockState, now: number): void {
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
		version(
			3,
			5 * 60 * MINUTE,
			`${d.contents.replace('instead of a generic internal error', 'and the position it failed at')}\nTo do: ask Core Engineer.\n`,
			'edited',
			'agent',
			'Core Engineer'
		),
		version(
			4,
			90 * MINUTE,
			`${d.contents}\nSee LIN-412 for the quoting rules.\n`,
			'edited',
			'extension',
			'Linear sync'
		),
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
