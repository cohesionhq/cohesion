<script lang="ts">
	import '$lib/tokens.css';
	import { goto, onNavigate, preloadCode } from '$app/navigation';
	import { page } from '$app/state';
	import { onMount, type Snippet } from 'svelte';
	import { cubicOut } from 'svelte/easing';
	import { slide } from 'svelte/transition';
	import { connection, onUnreachable, send, start } from '$lib/protocol/api';
	import { initSession, missingToken, onUnauthorized } from '$lib/protocol/session';
	import { trackBack } from '$lib/shell/back';
	import { fade, fadingScope } from '$lib/shell/fade';
	import { internalHref } from '$lib/shell/links';
	import { mainPane, panes } from '$lib/shell/pane.svelte';
	import PaneHost from '$lib/shell/PaneHost.svelte';
	import SplitView from '$lib/shell/SplitView.svelte';
	import Notice from '$lib/shell/Notice.svelte';
	import NewTaskDialog from '$lib/shell/NewTaskDialog.svelte';
	import { newTaskDialog } from '$lib/shell/newTaskDialog.svelte';
	import Palette from '$lib/shell/Palette.svelte';
	import { mainPages, routes } from '$lib/shell/routes';
	import {
		historyStepFor,
		historyStepForButton,
		isTyping,
		opensSettings,
		showsShortcuts,
		togglesSidebar,
		asksForNewDocument,
		asksForNewTask,
		workspaceIndexFor,
		shortcuts,
		focusesFolderHeading,
		folderHeadingButton,
		togglesEditing,
		isTypingInField
	} from '$lib/shell/shortcuts.svelte';
	import ForwardComposer from '$lib/features/agents/ForwardComposer.svelte';
	import LinkedItem from '$lib/shell/LinkedItem.svelte';
	import ShowKindsDialog from '$lib/features/connectors/ShowKindsDialog.svelte';
	import ShortcutsDialog from '$lib/shell/ShortcutsDialog.svelte';
	import Sidebar from '$lib/shell/Sidebar.svelte';
	import { sidebarState } from '$lib/shell/sidebarState.svelte';
	import PanelLeftOpen from '@lucide/svelte/icons/panel-left-open';
	import AgentGutter from '$lib/features/agents/AgentGutter.svelte';
	import SessionGutter from '$lib/features/actors/SessionGutter.svelte';
	import ConversationGutter from '$lib/shell/ConversationGutter.svelte';
	import CalendarList from '$lib/features/calendar/CalendarList.svelte';
	import WorkingMark from '$lib/shell/WorkingMark.svelte';

	import FolderDialogs from '$lib/shell/FolderDialogs.svelte';
	import DocumentMover from '$lib/features/documents/DocumentMover.svelte';
	import ConfirmDialog from '$lib/ui/ConfirmDialog.svelte';
	import type { ActorSession } from '$lib/protocol/types';
	import { isGoing } from '$lib/stores/actors.svelte';
	import { connectors } from '$lib/stores/connectors.svelte';
	import { refreshAll, refreshInvalidated } from '$lib/stores/resource.svelte';
	import { skills, skillsOf } from '$lib/stores/skills.svelte';
	import { modules } from '$lib/stores/modules.svelte';
	import { workspaces } from '$lib/stores/workspaces.svelte';
	import Toast from '$lib/ui/Toast.svelte';
	import { UNREACHABLE_MESSAGE, toasts } from '$lib/ui/toasts.svelte';
	import { notificationPreview, notify, openNotificationsWith } from '$lib/ui/notify';
	import { planOf } from '$lib/features/actors/plan';
	import { pageStores } from '$lib/stores/pageStores.svelte';

	const { actors, agents, resources, documents, tasks, tables } = pageStores();
	let mover = $state<DocumentMover>();

	/** Files a dropped document, or a table's row, through the mover. */
	function fileDocument(document_id: number, folder_id: number | null) {
		const ws = workspaces.currentId;
		if (ws === null) return;
		const row = tables.rows.value.find((r) => r.document_id === document_id);
		const doc =
			documents.list.value.find((d) => d.id === document_id) ??
			(row && {
				workspace_id: ws,
				id: row.document_id,
				folder_id: row.folder_id,
				title: row.title
			});
		if (doc) void mover?.move(doc, folder_id);
	}
	let resolvingSession = $state<ActorSession | null>(null);

	let { children }: { children: Snippet } = $props();

	/** Resolves `session`, asking first while its actor is still at work. */
	function resolveSession(session: ActorSession) {
		if (isGoing(session)) resolvingSession = session;
		else actors.resolve(session.id).catch((e) => toasts.error(e));
	}

	let ready = $state(false);
	let unauthorized = $state(false);
	let paletteOpen = $state(false);
	let choosingKinds = $state(false);
	let hostname = $state<string | null>(null);

	trackBack();

	/** The sidebar's slide, as long as the side panel's; none under reduced motion or on a phone, where it is a bar above the page. */
	const sidebarSlide = () => ({
		axis: 'x' as const,
		easing: cubicOut,
		duration: matchMedia('(prefers-reduced-motion: reduce), (max-width: 599.98px)').matches
			? 0
			: 180
	});

	const routeWorkspace = $derived(page.params.ws ? Number(page.params.ws) : null);
	const routeAgent = $derived(page.params.agent ? Number(page.params.agent) : null);
	const routeSession = $derived(page.params.session ? Number(page.params.session) : null);
	/** The agent whose conversation is open, which the column beside the page speaks for. */
	const routeGutterAgent = $derived(
		agents.list.value.find((agent) => agent.id === routeAgent) ?? null
	);
	/** The actor session open, which the column speaks for in the same way. */
	const routeGutterSession = $derived(
		routeSession !== null && actors.session.key === routeSession ? actors.session.value : null
	);
	/** Whether the open conversation's agent or session is still on its way. */
	const gutterLoading = $derived(
		routeAgent !== null
			? !agents.list.loaded
			: routeSession !== null && (actors.session.key !== routeSession || actors.session.loading)
	);
	// What the column speaks for. It keeps the last conversation while the next one loads, so
	// moving between two stays one open column whose contents fade (`shell/fade.ts`), rather than
	// one that shuts and opens again.
	let gutterAgent = $state<typeof routeGutterAgent>(null);
	let gutterSession = $state<typeof routeGutterSession>(null);
	$effect.pre(() => {
		const agent = routeGutterAgent;
		const session = routeGutterSession;
		if (gutterLoading) return;
		gutterAgent = agent;
		gutterSession = session;
	});

	/** The calendar page is open in the main half: the column lists its calendars. */
	const gutterCalendar = $derived(page.route.id === '/w/[ws]/calendar');

	// The route owns the current workspace; remember it for the next visit.
	$effect(() => {
		if (routeWorkspace !== null && !Number.isNaN(routeWorkspace)) {
			workspaces.select(routeWorkspace);
		}
	});

	// Outside a workspace route (Agents, Settings) the sidebar still shows the
	// last one, once the list loads, however many tries that takes.
	$effect(() => {
		if (workspaces.currentId === null && workspaces.list.loaded) {
			workspaces.currentId = workspaces.pick();
		}
	});

	// The main pages' code loads a moment after the app opens, so a first
	// visit (from a shortcut too, which no hover preloads) draws at once.
	$effect(() => {
		const ws = workspaces.currentId;
		if (ws === null) return;
		const timer = setTimeout(() => {
			for (const path of mainPages(ws)) preloadCode(path).catch(() => {});
		}, 1000);
		return () => clearTimeout(timer);
	});

	// The sidebar shows the current workspace's kinds of resource, in the
	// order the user put them, while a connector of theirs can import.
	$effect(() => {
		if (workspaces.currentId !== null) void resources.shownKinds.load(workspaces.currentId);
	});
	// The sidebar, ⌘K and a module's page leave out the modules the current workspace turned off.
	$effect(() => {
		if (workspaces.currentId !== null) void modules.current.load(workspaces.currentId);
	});
	// The sidebar lists the current workspace's pinned document and task folders; the Tasks
	// page's switcher, its saved views.
	$effect(() => {
		if (workspaces.currentId === null) return;
		void documents.folders.load(workspaces.currentId);
		void tasks.folders.load(workspaces.currentId);
		void tasks.views.load(workspaces.currentId);
	});
	const shownKinds = $derived.by(() => {
		const available = new Map(connectors.kinds.map((entry) => [entry.kind.id, entry]));
		return resources.shownKinds.value.flatMap(({ kind_id }) => available.get(kind_id) ?? []);
	});

	// A navigation of the main half fades it (`shell/fade.ts`); the second half fades its own.
	let mainRoot = $state<HTMLElement>();
	onNavigate((navigation) => {
		const target = mainRoot && fadingScope(mainRoot, navigation.from, navigation.to);
		if (!target) return;
		return fade(target, () => navigation.complete);
	});

	// "No split on phones": at a phone's width the split closes to the half last used.
	$effect(() => {
		const phone = matchMedia('(max-width: 599.98px)');
		const narrowed = () => {
			if (phone.matches && panes.split) panes.close(panes.focused === 'split' ? 'main' : 'split');
		};
		narrowed();
		phone.addEventListener('change', narrowed);
		return () => phone.removeEventListener('change', narrowed);
	});

	onMount(() => {
		initSession();
		if (missingToken()) {
			unauthorized = true;
			return;
		}
		const unsubscribe = [
			onUnauthorized(() => (unauthorized = true)),
			onUnreachable(() => toasts.show(UNREACHABLE_MESSAGE, 'error')),
			connection.onInvalidate((keys) => void refreshInvalidated(keys)),
			// A notification is stored nowhere: its key carries it, shown once as it arrives.
			connection.onInvalidate((keys) => {
				for (const key of keys) {
					if (key.kind === 'notification') {
						notify({ message: key.id.message, title: key.id.title });
					}
				}
			}),
			openNotificationsWith((href) => void goto(href)),
			connection.onReset(() => void refreshAll()),
			connection.onReconnected(() => void refreshAll())
		];
		start();
		ready = true;
		void workspaces.load();
		// Loaded app-wide so the sidebar's agents and new-message toasts work on every page.
		agents.onNewMessage = async (agent) => {
			const announcement = `${agent.agent_name} sent you a message.`;
			const content = await agents
				.latestText(agent.id)
				.then((text) => notificationPreview(text ?? ''))
				.catch(() => '');
			notify({
				// With the message's text as the body, the line that names the agent is the title.
				message: content || announcement,
				title: content ? announcement : undefined,
				href: routes.agent(agent.id),
				avatar: { name: agent.agent_name, color: agent.agent_color, emoji: agent.agent_emoji }
			});
		};
		void agents.loadList();
		void actors.load();
		void connectors.load();
		send('about')
			.then((about) => (hostname = about.hostname))
			.catch(() => undefined);

		/** ⌘N off the Documents page: a document in the current workspace's Inbox, opened to edit. */
		async function newDocumentInInbox() {
			const ws = workspaces.currentId;
			if (ws === null) return;
			try {
				const doc = await documents.create(ws);
				void panes.current.goto(routes.document(ws, doc.id));
			} catch (error) {
				toasts.error(error);
			}
		}

		const onFocus = () => void refreshAll();
		const onKey = (event: KeyboardEvent) => {
			if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
				event.preventDefault();
				paletteOpen = !paletteOpen;
				return;
			}
			if (togglesSidebar(event)) {
				event.preventDefault();
				sidebarState.toggle();
				return;
			}
			if (showsShortcuts(event)) {
				event.preventDefault();
				shortcuts.open = true;
				return;
			}
			if (opensSettings(event)) {
				// Captured, so neither a page nor the browser also acts on it.
				event.preventDefault();
				event.stopPropagation();
				void goto(routes.settings());
				return;
			}
			if (focusesFolderHeading(event)) {
				// A page without a folder heading leaves the key to the browser.
				const heading = folderHeadingButton();
				if (heading) {
					event.preventDefault();
					event.stopPropagation();
					heading.focus();
					// Opened, not only focused, so the arrow keys and Enter work at once.
					if (heading.getAttribute('aria-expanded') !== 'true') heading.click();
					return;
				}
			}
			// A page with no document editor, or a plain field, leaves the key alone.
			if (togglesEditing(event) && shortcuts.toggleEditing && !isTypingInField(event.target)) {
				event.preventDefault();
				event.stopPropagation();
				shortcuts.toggleEditing();
				return;
			}
			// ⌘N and ⌘T work in a field too, but not over a dialog, which has its own keys.
			const inDialog =
				event.target instanceof HTMLElement && event.target.closest('[role="dialog"]') !== null;
			if (!inDialog && asksForNewDocument(event)) {
				event.preventDefault();
				event.stopPropagation();
				// The Documents page makes it in the folder it shows; elsewhere it goes in the Inbox.
				if (shortcuts.newDocument) shortcuts.newDocument();
				else void newDocumentInInbox();
				return;
			}
			if (!inDialog && asksForNewTask(event)) {
				event.preventDefault();
				event.stopPropagation();
				if (shortcuts.newTask) shortcuts.newTask();
				else newTaskDialog.request();
				return;
			}
			const slot = workspaceIndexFor(event);
			if (slot !== null) {
				// A key with no workspace behind it is left alone.
				const target = workspaces.list.value[slot];
				if (!target) return;
				event.preventDefault();
				event.stopPropagation();
				void goto(routes.workspace(target.id));
				return;
			}
			const typing = isTyping(event.target);
			// Back and forward leave a field alone, where the same chords move the caret.
			const step = typing ? null : historyStepFor(event);
			if (step) {
				event.preventDefault();
				event.stopPropagation();
				panes.current.step(step);
				return;
			}
			if (event.metaKey || event.ctrlKey || event.altKey || typing) return;
			if (event.key === '?') {
				event.preventDefault();
				shortcuts.open = true;
			}
		};
		// A mouse's side buttons, which a browser takes itself but the WebUI harnesses' web view doesn't:
		// cancelling the press and the release keeps a browser from stepping a second time.
		const onButton = (event: MouseEvent) => {
			const step = historyStepForButton(event.button);
			if (!step) return;
			event.preventDefault();
			if (event.type === 'mouseup') panes.current.step(step);
		};
		// ⌥-click on a link opens it in the split view's second half, opening the split if it is closed.
		const onAltClick = (event: MouseEvent) => {
			if (!event.altKey || event.button !== 0 || matchMedia('(max-width: 599.98px)').matches)
				return;
			const href = internalHref(event.target);
			if (href === null) return;
			event.preventDefault();
			event.stopPropagation();
			panes.openSplit(href);
		};
		window.addEventListener('focus', onFocus);
		window.addEventListener('keydown', onKey, true);
		window.addEventListener('mousedown', onButton);
		window.addEventListener('mouseup', onButton);
		window.addEventListener('click', onAltClick, true);
		panes.restore();
		sidebarState.restore();
		return () => {
			for (const stop of unsubscribe) stop();
			window.removeEventListener('focus', onFocus);
			window.removeEventListener('keydown', onKey, true);
			window.removeEventListener('mousedown', onButton);
			window.removeEventListener('mouseup', onButton);
			window.removeEventListener('click', onAltClick, true);
		};
	});
</script>

{#if unauthorized}
	<Notice message="Open Cohesion with" code="cohesion open" />
{:else if connection.fatal}
	<Notice message={connection.fatal} />
{:else if ready}
	<!-- The page itself never scrolls; the sidebar and the main column each scroll on their own. -->
	<div class="flex h-dvh overflow-hidden max-sm:flex-col">
		{#if sidebarState.collapsed}
			<!-- Collapsed: only a restore button is left; ⌘K and the shortcuts do the navigating. -->
			<div
				transition:slide={sidebarSlide()}
				class="flex flex-none items-start px-1.5 pt-[calc(--spacing(3)+var(--titlebar-inset))] max-sm:px-2 max-sm:pt-[calc(--spacing(1)+var(--titlebar-inset))] max-sm:pb-1"
			>
				<button
					class="flex size-7.5 items-center justify-center rounded-sm text-fg-subtle hover:bg-bg-hover hover:text-fg"
					type="button"
					aria-label="Show sidebar"
					onclick={() => sidebarState.set(false)}
				>
					<PanelLeftOpen size={16} strokeWidth={1.75} />
				</button>
			</div>
		{:else}
			<div transition:slide={sidebarSlide()} class="flex flex-none max-sm:h-auto">
				<Sidebar
					pathname={page.url.pathname}
					search={page.url.search}
					workspaceId={workspaces.currentId}
					status={connection.status}
					{hostname}
					agents={agents.list.value}
					teams={agents.teams.value}
					actorSessions={actors.current}
					goingActorSessions={actors.goingCount}
					kinds={shownKinds}
					documentFolders={documents.folders.key === workspaces.currentId
						? documents.folders.value
						: []}
					taskFolders={tasks.folders.key === workspaces.currentId ? tasks.folders.value : []}
					onunpinfolder={(kind, id) =>
						workspaces.currentId !== null &&
						(kind === 'documents' ? documents : tasks)
							.setFolderPinned(workspaces.currentId, id, false)
							.catch((e) => toasts.error(e))}
					onfiledocument={fileDocument}
					onresolve={resolveSession}
					onsearch={() => (paletteOpen = true)}
					onchoosekinds={() => (choosingKinds = true)}
					onpin={(id, pinned) => agents.setPinned(id, pinned).catch((e) => toasts.error(e))}
					onreorder={(ids) => agents.reorder(ids).catch((e) => toasts.error(e))}
					onreorderkinds={(ids) =>
						workspaces.currentId !== null &&
						resources.reorderShown(workspaces.currentId, ids).catch((e) => toasts.error(e))}
					oncollapse={() => sidebarState.set(true)}
				/>
			</div>
		{/if}
		<!-- The page is a sheet inset in the window, the sidebar on the canvas around it. It reserves
		     no scrollbar gutter, so a bar across its top (a conversation's header) meets both edges
		     evenly; where scrollbars take room, a page that starts to scroll narrows by one.
		     The top margin keeps pages clear of a WebUI harness's window controls or status bar without scrolling
		     under them; on a phone-width screen the sidebar is a bar above the page and clears them instead.
		     With the sidebar collapsed nothing sits between the card and those controls, so the card starts a
		     sheet inset below them instead of flush with their bottom edge.
		     It holds one half (`PaneHost`), or two side by side in a split view, each its own scroller,
		     positioned, so what a page positions absolutely (an `sr-only` label in each of a thousand
		     rows) scrolls in there rather than stretching the document into another scroller.
		     Each scroller is the outermost scope its half's navigations cross-fade (`shell/fade.ts`);
		     the sheet that draws the card sits outside them, so the card stays put while only what is
		     on it dissolves. -->
		<div
			class={[
				'm-(--sheet-inset) ml-0 flex min-h-0 min-w-0 flex-1 rounded-(--sheet-radius) bg-bg shadow-sheet transition-[margin-top] max-sm:m-0 max-sm:shadow-none',
				sidebarState.collapsed
					? 'mt-[calc(var(--sheet-inset)+var(--titlebar-inset))]'
					: 'mt-[max(var(--sheet-inset),var(--titlebar-inset))]'
			]}
		>
			<PaneHost side="main" pane={mainPane} bind:root={mainRoot}>
				{@render children()}
			</PaneHost>
			{#if panes.split}
				<SplitView split={panes.split} onsearch={() => (paletteOpen = true)} />
			{/if}
		</div>
		<ConversationGutter
			open={gutterAgent !== null || gutterSession !== null || gutterCalendar}
			label={gutterCalendar ? 'Calendars' : undefined}
		>
			{#if gutterCalendar}
				<CalendarList workspaceId={workspaces.currentId} summary />
			{:else if gutterAgent}
				{@const agentId = gutterAgent.id}
				<AgentGutter
					agent={gutterAgent}
					cues={agents.cues.key === gutterAgent.id ? agents.cues.value : []}
					agenda={agents.agenda.key === gutterAgent.id ? agents.agenda.value : []}
					skills={skillsOf(skills.list.value, gutterAgent)}
					onsendskill={(skill) =>
						agents.sendMessage(agentId, '', [], undefined, skill).catch((e) => toasts.error(e))}
					actors={actors.list.value}
					sessions={actors.followedBy(gutterAgent.id)}
					onclearagenda={() => agents.resetAgenda(agentId).catch((e) => toasts.error(e))}
					onresolve={resolveSession}
					onunfollow={(session) =>
						actors.unfollow(session.id, agentId).catch((e) => toasts.error(e))}
				/>
			{:else if gutterSession}
				{@const session = gutterSession}
				<SessionGutter
					{session}
					actor={actors.list.value.find((a) => a.id === session.actor_id) ?? null}
					plan={planOf(actors.usage.value, session.actor_id)}
					above={session.subagent && actors.above.key === session.subagent.parent_session_id
						? actors.above.value
						: null}
					subagents={actors.subagents.key === session.id ? actors.subagents.value : []}
					monitor={agents.list.value.find((a) => a.id === session.follower?.agent_id) ?? null}
					onunfollow={(agent) =>
						actors.unfollow(session.id, agent.id).catch((e) => toasts.error(e))}
				/>
			{/if}
		</ConversationGutter>
	</div>
	<FolderDialogs />
	<NewTaskDialog />
	<DocumentMover bind:this={mover} onmoved={(doc) => tables.took(doc)} />
	<Palette bind:open={paletteOpen} kinds={shownKinds} />
	{#if workspaces.currentId !== null}
		<ShowKindsDialog
			bind:open={choosingKinds}
			workspaceId={workspaces.currentId}
			shown={shownKinds}
		/>
	{/if}
	<ShortcutsDialog />
	<ForwardComposer />
	<LinkedItem />
	<ConfirmDialog
		subject={resolvingSession}
		onclose={() => (resolvingSession = null)}
		title="Resolve “{resolvingSession?.title ?? ''}”?"
		description="The actor will stop what it’s doing. This session will be removed from your list. To pick it back up, send a message."
		confirmLabel="Resolve"
		onconfirm={(session) => actors.resolve(session.id)}
	/>
	<Toast />
	<WorkingMark />
{/if}
