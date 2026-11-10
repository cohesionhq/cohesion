<!--
	⌘K palette, the way to every page: there are no chords that jump to one. Root mode lists routes
	and actions, the current workspace's task folders and the kinds of resource its sidebar shows,
	every settings section, every feed and every agent (drawn with its avatar), and the workspaces whose title matches (filtered
	locally), then live search results
	from the current workspace and among actor sessions; "New task" and "Switch workspace…" switch the palette into a sub-mode
	instead of opening another dialog. A pick opens in the half of the sheet in use; with ⌥ held it
	opens in the split view's second half.
-->
<script lang="ts">
	import { Command, Dialog } from 'bits-ui';
	import SquareTerminal from '@lucide/svelte/icons/square-terminal';
	import Table2 from '@lucide/svelte/icons/table-2';
	import Bot from '@lucide/svelte/icons/bot';
	import BookUser from '@lucide/svelte/icons/book-user';
	import History from '@lucide/svelte/icons/history';
	import Cable from '@lucide/svelte/icons/cable';
	import CalendarDays from '@lucide/svelte/icons/calendar-days';
	import CheckSquare from '@lucide/svelte/icons/square-check';
	import FilePlus from '@lucide/svelte/icons/file-plus';
	import FileText from '@lucide/svelte/icons/file-text';
	import Folder from '@lucide/svelte/icons/folder';
	import FolderPlus from '@lucide/svelte/icons/folder-plus';
	import Layers from '@lucide/svelte/icons/layers';
	import LayoutDashboard from '@lucide/svelte/icons/layout-dashboard';
	import Keyboard from '@lucide/svelte/icons/keyboard';
	import ListPlus from '@lucide/svelte/icons/list-plus';
	import Newspaper from '@lucide/svelte/icons/newspaper';
	import Rss from '@lucide/svelte/icons/rss';
	import Telescope from '@lucide/svelte/icons/telescope';
	import NotebookPen from '@lucide/svelte/icons/notebook-pen';
	import PackageSearch from '@lucide/svelte/icons/package-search';
	import Puzzle from '@lucide/svelte/icons/puzzle';
	import Repeat from '@lucide/svelte/icons/repeat';
	import Settings from '@lucide/svelte/icons/settings';
	import { untrack, type Component } from 'svelte';
	import { SECTIONS } from '$lib/features/settings/sections';
	import { matchingLine, prefixQuery } from '$lib/format/search';
	import { send } from '$lib/protocol/api';
	import type {
		ActorSessionSearchHit,
		Connector,
		IntelligenceAgent,
		SearchResult
	} from '$lib/protocol/types';
	import { keyLabel } from '$lib/shell/keys';
	import { panes } from '$lib/shell/pane.svelte';
	import { routes } from '$lib/shell/routes';
	import { shortcuts } from '$lib/shell/shortcuts.svelte';
	import type { ConnectorKind } from '$lib/stores/connectors.svelte';
	import { viewParam } from '$lib/stores/articles.svelte';
	import { feeds } from '$lib/stores/feeds.svelte';
	import { workspaces } from '$lib/stores/workspaces.svelte';
	import AgentAvatar from '$lib/ui/AgentAvatar.svelte';
	import IconTile from '$lib/ui/IconTile.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';
	import { pageStores } from '$lib/stores/pageStores.svelte';

	const { agents, tasks } = pageStores();

	interface Props {
		open?: boolean;
		/** The kinds of resource the current workspace's sidebar shows, each listed as a page. */
		kinds?: ConnectorKind[];
	}

	let { open = $bindable(false), kinds = [] }: Props = $props();

	type Mode = 'root' | 'newTask' | 'switch';

	interface Entry {
		id: string;
		label: string;
		icon: Component;
		/** Drawn with this agent's avatar in place of the icon. */
		agent?: IntelligenceAgent;
		/** Drawn with this connector's icon in place of the icon, as the sidebar draws a kind. */
		connector?: Connector;
		run: () => void | Promise<void>;
	}

	let mode = $state<Mode>('root');
	let query = $state('');
	let results = $state<SearchResult[]>([]);
	let actorResults = $state<ActorSessionSearchHit[]>([]);
	let searchTimer: ReturnType<typeof setTimeout> | null = null;
	let searchGeneration = 0;

	const ws = $derived(workspaces.currentId);

	function close() {
		open = false;
	}

	/** Whether ⌥ was held for the pick (↩ or a click), which opens it in the split view. */
	let toSplit = false;
	function noteAlt(event: KeyboardEvent | PointerEvent) {
		toSplit = event.altKey && !matchMedia('(max-width: 599.98px)').matches;
	}

	function go(path: string) {
		close();
		if (toSplit) panes.openSplit(path);
		else void panes.current.goto(path);
	}

	const entries = $derived.by((): Entry[] => {
		const list: Entry[] = [];
		if (ws !== null) {
			const id = ws;
			list.push(
				{
					id: 'dashboard',
					label: 'Dashboard',
					icon: LayoutDashboard,
					run: () => go(routes.dashboard(id))
				},
				{ id: 'today', label: 'Today', icon: CalendarDays, run: () => go(routes.today(id)) },
				{
					id: 'journal',
					label: 'Journal',
					icon: NotebookPen,
					run: () => go(routes.journal(id))
				},
				{
					id: 'documents',
					label: 'Documents',
					icon: FileText,
					run: () => go(routes.documents(id))
				},
				{ id: 'tasks', label: 'Tasks', icon: CheckSquare, run: () => go(routes.tasks(id)) },
				{
					id: 'databases',
					label: 'Databases',
					icon: Table2,
					run: () => go(routes.databases(id))
				},
				{ id: 'articles', label: 'Articles', icon: Newspaper, run: () => go(routes.articles(id)) },
				{ id: 'contacts', label: 'Contacts', icon: BookUser, run: () => go(routes.contacts(id)) },
				{ id: 'rewind', label: 'Rewind', icon: History, run: () => go(routes.rewind(id)) },
				...kinds.map(({ connector, kind }) => ({
					id: `kind-${kind.id}`,
					label: kind.plural,
					icon: Layers,
					connector,
					run: () => go(routes.resources(id, kind.id))
				})),
				{
					id: 'new-task',
					label: 'New task',
					icon: ListPlus,
					run: () => enter('newTask')
				},
				{ id: 'new-document', label: 'New document', icon: FilePlus, run: newDocument },
				{
					id: 'add-feed',
					label: 'Add feed…',
					icon: Rss,
					// The Articles page opens its add-feed dialog for `?add=feed`.
					run: () => go(routes.addFeed(id))
				},
				{ id: 'research', label: 'Research', icon: Telescope, run: () => go(routes.research(id)) },
				{
					id: 'new-folder',
					label: 'New folder…',
					icon: FolderPlus,
					// The Tasks page opens its folder dialog for `?new=folder`.
					run: () => go(`${routes.tasks(id)}?new=folder`)
				},
				...(tasks.folders.key === id ? tasks.folders.value : []).map((folder) => ({
					id: `task-folder-${folder.id}`,
					label: folder.title,
					icon: Folder,
					run: () => go(`${routes.tasks(id)}?view=${folder.id}`)
				})),
				...feeds.list.value.map((feed) => ({
					id: `feed-${feed.id}`,
					label: feed.title,
					icon: Rss,
					run: () => go(routes.articles(id, viewParam({ feed: feed.id })))
				}))
			);
		}
		list.push(
			{ id: 'agents', label: 'Agents', icon: Bot, run: () => go(routes.agents()) },
			{ id: 'new-agent', label: 'New agent…', icon: Bot, run: () => go(routes.newAgent()) },
			...agents.list.value.map((agent) => ({
				id: `agent-${agent.id}`,
				label: agent.agent_name,
				icon: Bot,
				agent,
				run: () => go(routes.agent(agent.id))
			})),
			{
				id: 'actors',
				label: 'Actors',
				icon: SquareTerminal,
				run: () => go(routes.actors())
			},
			{
				id: 'connector-accounts',
				label: 'Accounts',
				icon: Cable,
				run: () => go(routes.connectorAccounts())
			},
			{
				id: 'extensions-browse',
				label: 'Browse extensions',
				icon: PackageSearch,
				run: () => go(routes.extensionsBrowse())
			},
			{
				id: 'extensions-installed',
				label: 'Installed extensions',
				icon: Puzzle,
				run: () => go(routes.extensionsInstalled())
			},
			{ id: 'settings', label: 'Settings', icon: Settings, run: () => go(routes.settings()) },
			...SECTIONS.map((section) => ({
				id: `settings-${section.slug}`,
				label: `Settings › ${section.label}`,
				icon: Settings,
				run: () => go(routes.settings(section.slug))
			})),
			{
				id: 'shortcuts',
				label: 'Keyboard shortcuts',
				icon: Keyboard,
				run: () => {
					close();
					shortcuts.open = true;
				}
			},
			{
				id: 'switch',
				label: 'Switch workspace…',
				icon: Repeat,
				run: () => enter('switch')
			}
		);
		return list;
	});

	const filtered = $derived.by(() => {
		const q = query.trim().toLowerCase();
		return q === '' ? entries : entries.filter((e) => e.label.toLowerCase().includes(q));
	});

	const switchable = $derived.by(() => {
		const q = query.trim().toLowerCase();
		return workspaces.list.value.filter((w) => q === '' || w.title.toLowerCase().includes(q));
	});

	/** Workspaces among root mode's matches, once something is typed. */
	const workspaceMatches = $derived(query.trim() === '' ? [] : switchable);

	function enter(next: Mode) {
		mode = next;
		query = '';
		results = [];
	}

	async function newDocument() {
		if (ws === null) return;
		const id = ws;
		try {
			const doc = await send('createDocument', {
				workspace_id: id,
				title: 'Untitled',
				contents: ''
			});
			go(routes.document(id, doc.id));
		} catch (error) {
			toasts.error(error);
		}
	}

	async function createTask() {
		const title = query.trim();
		if (ws === null || title === '') return;
		try {
			await send('createTask', { workspace_id: ws, title, folder_id: null });
			toasts.show('Added to the inbox.');
			close();
		} catch (error) {
			toasts.error(error);
		}
	}

	/** A resource's page names its kind, which a search hit doesn't carry. */
	async function openResource(result: SearchResult) {
		try {
			const resource = await send('getResource', {
				workspace_id: result.workspace_id,
				resource_id: result.entity_id
			});
			go(routes.resource(resource.workspace_id, resource.kind_id, resource.id));
		} catch (error) {
			toasts.error(error);
		}
	}

	function openResult(result: SearchResult) {
		if (result.entity_kind === 'document')
			go(routes.document(result.workspace_id, result.entity_id));
		else if (result.entity_kind === 'resource') void openResource(result);
		else if (result.entity_kind === 'contact')
			go(routes.contact(result.workspace_id, result.entity_id));
		else go(routes.task(result.workspace_id, result.entity_id));
	}

	// Live search, debounced; a newer query drops older answers.
	$effect(() => {
		const q = query.trim();
		const fts = prefixQuery(q);
		const id = ws;
		if (searchTimer) clearTimeout(searchTimer);
		if (mode !== 'root' || fts === null || q.length < 2) {
			results = [];
			actorResults = [];
			return;
		}
		const generation = ++searchGeneration;
		// Both answers land on their own; a failed search shows nothing.
		const answer = <T,>(search: Promise<T[]>, take: (found: T[]) => void) =>
			search.then(
				(found) => generation === searchGeneration && take(found),
				() => generation === searchGeneration && take([])
			);
		searchTimer = setTimeout(() => {
			if (id !== null)
				void answer(
					send('searchWorkspace', { workspace_id: id, query: fts }),
					(found) => (results = found.slice(0, 12))
				);
			else results = [];
			void answer(
				send('searchActorSessions', { query: fts }),
				(found) => (actorResults = found.slice(0, 6))
			);
		}, 150);
	});

	// Every open starts fresh at the root, with the folders and feeds it lists.
	$effect(() => {
		if (open) {
			mode = 'root';
			query = '';
			results = [];
			actorResults = [];
			if (ws !== null) void untrack(() => tasks.folders.load(ws));
			void untrack(() => feeds.load());
		}
	});

	function onkeydown(event: KeyboardEvent) {
		if (mode === 'newTask' && event.key === 'Enter') {
			event.preventDefault();
			void createTask();
		} else if (mode !== 'root' && event.key === 'Backspace' && query === '') {
			event.preventDefault();
			enter('root');
		}
	}

	const itemClass =
		'flex cursor-pointer items-center gap-2 overflow-hidden rounded-sm p-2 whitespace-nowrap text-fg data-selected:bg-bg-hover [&_svg]:flex-none [&_svg]:text-fg-subtle';
	const emptyClass = 'px-2 py-3 text-fg-subtle';
	const headingClass = 'px-2 pt-2 pb-1 text-xs text-fg-subtle';
	const titleClass = 'max-w-[50%] flex-none overflow-hidden text-ellipsis';
	const snippetClass = 'min-w-0 overflow-hidden text-xs text-ellipsis text-fg-subtle';

	const placeholder = $derived(
		mode === 'newTask'
			? 'Task title, then Enter'
			: mode === 'switch'
				? 'Switch to workspace…'
				: 'Search or jump to…'
	);
</script>

<svelte:window onkeydowncapture={noteAlt} onpointerdowncapture={noteAlt} />

<Dialog.Root bind:open>
	<Dialog.Portal>
		<Dialog.Overlay class="overlay-backdrop" />
		<Dialog.Content
			class="overlay-surface fixed top-[15vh] left-1/2 z-50 w-[min(560px,calc(100vw-2*var(--gutter)))] -translate-x-1/2 overflow-hidden"
		>
			<Dialog.Title class="visually-hidden">Command palette</Dialog.Title>
			<Command.Root shouldFilter={false} loop>
				<Command.Input
					class="h-11 w-full border-b border-border px-4 text-md text-fg focus:shadow-none focus:outline-none"
					{placeholder}
					bind:value={query}
					{onkeydown}
				/>
				<Command.List class="max-h-[min(360px,60vh)] overflow-y-auto p-1">
					<Command.Viewport>
						{#if mode === 'root'}
							{#if filtered.length > 0}
								<Command.Group value="actions">
									<Command.GroupItems>
										{#each filtered as entry (entry.id)}
											{@const Icon = entry.icon}
											<Command.Item class={itemClass} value={entry.id} onSelect={entry.run}>
												{#if entry.agent}
													<!-- A 20px avatar in the 15px icon slot, as the sidebar draws it. -->
													<span class="-mx-0.5 flex">
														<AgentAvatar
															name={entry.agent.agent_name}
															color={entry.agent.agent_color}
															emoji={entry.agent.agent_emoji}
															size="xs"
														/>
													</span>
												{:else if entry.connector}
													<IconTile
														icon={entry.connector.icon}
														title={entry.connector.title}
														size={15}
													/>
												{:else}
													<Icon size={15} strokeWidth={1.75} />
												{/if}
												<span>{entry.label}</span>
											</Command.Item>
										{/each}
									</Command.GroupItems>
								</Command.Group>
							{/if}
							{#if workspaceMatches.length > 0}
								<Command.Group value="workspaces">
									<Command.GroupHeading class={headingClass}>Workspaces</Command.GroupHeading>
									<Command.GroupItems>
										{#each workspaceMatches as workspace (workspace.id)}
											<Command.Item
												class={itemClass}
												value={`workspace-${workspace.id}`}
												onSelect={() => go(routes.workspace(workspace.id))}
											>
												<Layers size={15} strokeWidth={1.75} />
												<span class={titleClass}>{workspace.title}</span>
												{#if workspace.id === ws}<span class={snippetClass}>Current</span>{/if}
											</Command.Item>
										{/each}
									</Command.GroupItems>
								</Command.Group>
							{/if}
							{#if results.length > 0}
								<Command.Group value="results">
									<Command.GroupHeading class={headingClass}>
										In {workspaces.current?.title ?? 'this workspace'}
									</Command.GroupHeading>
									<Command.GroupItems>
										{#each results as result (result.entity_kind + result.entity_id)}
											<Command.Item
												class={itemClass}
												value={`result-${result.entity_kind}-${result.entity_id}`}
												onSelect={() => openResult(result)}
											>
												{#if result.entity_kind === 'document'}
													<FileText size={15} strokeWidth={1.75} />
												{:else if result.entity_kind === 'resource'}
													<Layers size={15} strokeWidth={1.75} />
												{:else if result.entity_kind === 'contact'}
													<BookUser size={15} strokeWidth={1.75} />
												{:else}
													<CheckSquare size={15} strokeWidth={1.75} />
												{/if}
												<span class={titleClass}>{result.title ?? result.contents}</span>
												{#if result.title && result.contents}
													<span class={snippetClass}>{matchingLine(result.contents, query)}</span>
												{/if}
											</Command.Item>
										{/each}
									</Command.GroupItems>
								</Command.Group>
							{/if}
							{#if actorResults.length > 0}
								<Command.Group value="actor-results">
									<Command.GroupHeading class={headingClass}>Actor sessions</Command.GroupHeading>
									<Command.GroupItems>
										{#each actorResults as hit (hit.id)}
											<Command.Item
												class={itemClass}
												value={`actor-session-${hit.id}`}
												onSelect={() => go(routes.actorSession(hit.id))}
											>
												<SquareTerminal size={15} strokeWidth={1.75} />
												<span class={titleClass}>{hit.title}</span>
												{#if hit.excerpt}
													<span class={snippetClass}>{hit.excerpt}</span>
												{/if}
											</Command.Item>
										{/each}
									</Command.GroupItems>
								</Command.Group>
							{/if}
						{:else if mode === 'switch'}
							{#each switchable as workspace (workspace.id)}
								<Command.Item
									class={itemClass}
									value={`ws-${workspace.id}`}
									onSelect={() => go(routes.workspace(workspace.id))}
								>
									<span>{workspace.title}</span>
									{#if workspace.id === ws}<span class={snippetClass}>Current</span>{/if}
								</Command.Item>
							{:else}
								<Command.Empty class={emptyClass}>No matches.</Command.Empty>
							{/each}
						{:else}
							<p class={emptyClass}>
								Adds to the {workspaces.current?.title ?? 'workspace'} inbox. Backspace to go back.
							</p>
						{/if}
					</Command.Viewport>
				</Command.List>
				{#if mode === 'root'}
					<p
						class="flex items-center gap-1.5 border-t border-border px-4 py-2 text-xs text-fg-subtle max-sm:hidden"
					>
						<kbd>{keyLabel('alt', 'enter')}</kbd> Open in split view
					</p>
				{/if}
			</Command.Root>
		</Dialog.Content>
	</Dialog.Portal>
</Dialog.Root>
