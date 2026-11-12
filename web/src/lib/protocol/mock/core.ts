// What every area of the mock daemon shares: its state, the lookups that find
// a row or say it isn't there, and the invalidations it pushes after a write.
// Each file in `handlers/` takes what it needs from here.

import { parseItemLink } from '$lib/format/itemLinks';
import { ProtocolError } from '../errors';
import type {
	ActorSession,
	AgentSkill,
	AgentSkillScope,
	Article,
	ArticleFetchFailure,
	ArticleFilter,
	ArticleSummary,
	Client,
	Contact,
	ContactMethod,
	ContactMethodKind,
	Document,
	DocumentFolder,
	DocumentFolderProperty,
	Embed,
	EndpointModel,
	ExtensionPermissionKind,
	ExtensionUpdate,
	Feed,
	FeedSummary,
	FormulaDefinition,
	ImageUpload,
	IntelligenceAgent,
	Invalidation,
	NewPropertyOption,
	PropertyDataType,
	PropertyOption,
	RedactedConnectorAccount,
	RefetchedArticle,
	Replies,
	RequestType,
	Requests,
	SavedTaskView,
	SecurityEventKind,
	Tag,
	TagFilter,
	Task,
	Version,
	VersionChange,
	VersionSummary
} from '../types';
import { fromCatalog, securityEvent, seed, seedRewind, seedTables } from './seed';
import type {
	MockCapture,
	MockConnector,
	MockConnectorAccount,
	MockContact,
	MockContactMethod,
	MockSource
} from './state';
import { fallbackTitle, nowSeconds } from './support';
import {
	computeFormulas,
	folderDocuments,
	folderProperties,
	nextOptionId,
	tableRow
} from './tableRules';

/** One handler per request kind, which `createMock` checks is complete. */
export type Handlers = { [K in RequestType]: (args: Requests[K]) => Replies[K] };

/** The state and shared lookups of one mock daemon. */
export type MockCore = ReturnType<typeof createCore>;

export function createCore() {
	const state = seed();
	seedRewind(state, nowSeconds());
	seedTables(state);
	const nextId = (rows: { id: number }[]) =>
		rows.reduce((max, row) => Math.max(max, row.id), 0) + 1;
	const notFound = (what: string): never => {
		throw new ProtocolError('notFound', `The requested ${what} wasn’t found.`);
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
	/** A live folder as the daemon lists it. */
	const listedFolder = (f: DocumentFolder): DocumentFolder => ({ ...f });
	const tableOf = (workspace_id: number, folder_id: number) => {
		workspace(workspace_id);
		return documentFolder(workspace_id, folder_id);
	};
	const propertyOf = (workspace_id: number, folder_id: number, property_id: number) => {
		tableOf(workspace_id, folder_id);
		return (
			state.folderProperties.find((p) => p.id === property_id && p.folder_id === folder_id) ??
			notFound('document folder property')
		);
	};
	/** Live document `document_id` of folder `folder_id`. */
	const rowOf = (workspace_id: number, folder_id: number, document_id: number) => {
		tableOf(workspace_id, folder_id);
		const d = document(workspace_id, document_id);
		return d.folder_id === folder_id ? d : notFound('document');
	};
	// The daemon pushes what went stale after a write; the mock pushes the keys
	// of tables (`documentTable`, and `documentFolders` as a folder becomes a
	// table or stops being one), so the pages showing them refetch.
	const invalidateListeners = new Set<(keys: Invalidation[]) => void>();
	const invalidate = (...keys: Invalidation[]) => {
		setTimeout(() => {
			for (const listener of invalidateListeners) listener(keys);
		}, 0);
	};
	/** Pushes `documentTable` of each of `folders` that is a folder. */
	const tablesChanged = (...folders: (number | null)[]) => {
		const ids = [...new Set(folders.filter((f): f is number => f !== null))];
		if (ids.length) invalidate(...ids.map((id) => ({ kind: 'documentTable' as const, id })));
	};
	const requirePropertyName = (name: string) => {
		if (name === '') throw new ProtocolError('validationFailed', 'Name can’t be empty.', 'name');
	};
	const requireFormula = (formula: FormulaDefinition | null) => {
		if (formula !== null && formula.source.trim() === '')
			throw new ProtocolError('validationFailed', 'A formula can’t be empty.', 'formula');
	};
	const requireNotFormulaChoice = (data_type: PropertyDataType, formula: boolean) => {
		if (data_type === 'choice' && formula)
			throw new ProtocolError('validationFailed', 'A formula can’t be a choice.', 'data_type');
	};
	/** Puts `next` where property `p` was, as a retyped property is a new object. */
	const replaceProperty = (p: DocumentFolderProperty, next: DocumentFolderProperty) => {
		state.folderProperties = state.folderProperties.map((x) => (x.id === p.id ? next : x));
		return next;
	};
	/** Choice property `property_id`; another type has no options to change. */
	const choiceOf = (workspace_id: number, folder_id: number, property_id: number) => {
		const p = propertyOf(workspace_id, folder_id, property_id);
		if (p.data_type !== 'choice')
			throw new ProtocolError(
				'validationFailed',
				'Only a choice property has options.',
				'property_id'
			);
		return p;
	};
	/** Refuses an empty option name, or one another of `options` has ignoring case. */
	const requireOptionName = (
		options: PropertyOption[],
		name: string,
		field: string,
		except: number | null = null
	) => {
		if (name === '') throw new ProtocolError('validationFailed', 'Name can’t be empty.', field);
		if (options.some((o) => o.id !== except && o.name.toLowerCase() === name.toLowerCase()))
			throw new ProtocolError(
				'validationFailed',
				`This column already has an option named “${name}”.`,
				field
			);
	};
	/** Appends `added` to choice `p`'s options, each named uniquely. */
	const addOptions = (
		p: DocumentFolderProperty & { data_type: 'choice' },
		added: NewPropertyOption[],
		field: string
	) => {
		for (const { name, color = null } of added) {
			requireOptionName(p.options, name, field);
			p.options.push({
				// Its own options count too: a new property isn't in the state yet.
				id: Math.max(nextOptionId(state), ...p.options.map((o) => o.id + 1)),
				property_id: p.id,
				name,
				color,
				sort_order: p.options.reduce((max, o) => Math.max(max, o.sort_order), -1) + 1
			});
		}
	};
	/** Option `option_id` of choice `p`. */
	const optionIn = (p: DocumentFolderProperty & { data_type: 'choice' }, option_id: number) =>
		p.options.find((o) => o.id === option_id) ?? notFound('property option');
	/** The reply to an option command: the property, after the table is told. */
	const optionsChanged = (folder_id: number, p: DocumentFolderProperty) => {
		computeFormulas(state, folder_id);
		tablesChanged(folder_id);
		return structuredClone(p);
	};
	// A contact as the daemon reads it: its row, with its methods by id.
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
		const target = parseItemLink(link.trim());
		if (target?.kind === 'document-folder') {
			const folder = tableOf(target.ws, target.id);
			const docs = folderDocuments(state, folder.id).filter((d) => !d.deleted);
			return {
				link: `cohesion://document-folder/${target.ws}/${target.id}`,
				kind: 'documentFolder',
				workspace_id: target.ws,
				title: folder.title,
				excerpt: '',
				item: {
					kind: 'documentFolder',
					folder: listedFolder(folder),
					properties: folderProperties(state, folder.id).map((p) => structuredClone(p)),
					rows: docs.slice(0, 5).map((d) => tableRow(state, d)),
					row_count: docs.length
				}
			};
		}
		if (target?.kind === 'contact') {
			workspace(target.ws);
			const contact = withMethods(contactRow(target.ws, target.id));
			return {
				link: `cohesion://contact/${target.ws}/${target.id}`,
				kind: 'contact',
				workspace_id: target.ws,
				title: [contact.first_name, contact.middle_name, contact.last_name]
					.filter(Boolean)
					.join(' '),
				excerpt: excerptOf(contact.notes),
				item: { kind: 'contact', contact }
			};
		}
		if (target === null)
			throw new ProtocolError(
				'validationFailed',
				'That isn’t a link to a Cohesion item, such as cohesion://task/3/42.',
				'link'
			);
		const { kind, id, ws } = target;
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
			folder_id: null,
			pinned: false
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
	/** A skill's fields as the daemon stores them: trimmed, with a name and a prompt, and the name free in `scope`. */
	const skillFields = (
		scope: AgentSkillScope,
		fields: { name: string; description: string; prompt: string; agent_can_use: boolean },
		except?: number
	) => {
		const name = fields.name.trim();
		if (name === '') throw new ProtocolError('validationFailed', 'A skill needs a name.', 'name');
		const prompt = fields.prompt.trim();
		if (prompt === '')
			throw new ProtocolError('validationFailed', 'A skill needs a prompt.', 'prompt');
		const sameScope = (other: AgentSkillScope) =>
			other.kind === 'agent' && scope.kind === 'agent'
				? other.agent_id === scope.agent_id
				: other.kind === 'team' && scope.kind === 'team' && other.team_id === scope.team_id;
		if (
			state.agentSkills.some(
				(s) => s.id !== except && sameScope(s.scope) && s.name.toLowerCase() === name.toLowerCase()
			)
		)
			throw new ProtocolError(
				'validationFailed',
				`There's already a skill named “${name}” here.`,
				'name'
			);
		return {
			name,
			description: fields.description.trim(),
			prompt,
			agent_can_use: fields.agent_can_use
		};
	};
	const byName = (a: AgentSkill, b: AgentSkill) =>
		a.name.localeCompare(b.name, undefined, { sensitivity: 'base' }) || a.id - b.id;

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
							'rebuiltContext',
							'agenda'
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
		for (const a of state.agents)
			if (a.endpoint_model_id != null && gone.has(a.endpoint_model_id)) a.endpoint_model_id = null;
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
	return {
		state,
		nextId,
		notFound,
		workspace,
		document,
		liveDocumentFolders,
		documentFolder,
		documentSubtree,
		listedFolder,
		tableOf,
		propertyOf,
		rowOf,
		invalidateListeners,
		invalidate,
		tablesChanged,
		requirePropertyName,
		requireFormula,
		requireNotFormulaChoice,
		replaceProperty,
		choiceOf,
		requireOptionName,
		addOptions,
		optionIn,
		optionsChanged,
		contactRow,
		withMethods,
		contactMethod,
		contactNames,
		requireMethodValue,
		touchContact,
		addMethod,
		methodReply,
		task,
		liveFolders,
		folder,
		savedViews,
		savedView,
		ensureViewName,
		ensureViewFilters,
		feed,
		liveFeeds,
		feedSummary,
		removeItems,
		summary,
		tagIdsOf,
		tagsOf,
		tagOf,
		ensureTagName,
		withTags,
		coveredBy,
		words,
		excerptOf,
		linksTo,
		resolveLink,
		shownKinds,
		grantedRegistries,
		extensionUpdates,
		journalFor,
		joins,
		itemKey,
		keep,
		keepDocument,
		keepTask,
		versionedDeleted,
		versionSummary,
		notFoundVersion,
		touch,
		requireTitle,
		emojiValue,
		agent,
		agentRow,
		agentTeam,
		skillFields,
		byName,
		teamTitle,
		sortedTeams,
		agentActivity,
		sortedAgents,
		agentMessageById,
		preview,
		granted,
		connector,
		connectorAccount,
		definition,
		redacted,
		served,
		clientServes,
		actorSession,
		subagentsBelow,
		startedSession,
		attachActorImages,
		fetchPage,
		record,
		article,
		resource,
		remoteResource,
		orphan,
		modelsOf,
		dropModels,
		addEndpointModel,
		recordSecurity,
		pairingRefusal,
		revokeClient
	};
}
