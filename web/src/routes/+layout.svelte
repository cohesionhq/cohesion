<script lang="ts">
	import '$lib/tokens.css';
	import { goto, onNavigate } from '$app/navigation';
	import { page } from '$app/state';
	import { onMount, type Snippet } from 'svelte';
	import { connection, onUnreachable, send, start } from '$lib/protocol/api';
	import { initSession, missingToken, onUnauthorized } from '$lib/protocol/session';
	import { trackBack } from '$lib/shell/back';
	import { fade, fadingScope } from '$lib/shell/fade';
	import { internalHref } from '$lib/shell/links';
	import { mainPane, panes } from '$lib/shell/pane.svelte';
	import PaneHost from '$lib/shell/PaneHost.svelte';
	import SplitView from '$lib/shell/SplitView.svelte';
	import Notice from '$lib/shell/Notice.svelte';
	import Palette from '$lib/shell/Palette.svelte';
	import { routes } from '$lib/shell/routes';
	import {
		historyStepFor,
		historyStepForButton,
		isTyping,
		opensSettings,
		showsShortcuts,
		shortcuts
	} from '$lib/shell/shortcuts.svelte';
	import ForwardComposer from '$lib/features/agents/ForwardComposer.svelte';
	import LinkedItem from '$lib/shell/LinkedItem.svelte';
	import ShowKindsDialog from '$lib/features/connectors/ShowKindsDialog.svelte';
	import ShortcutsDialog from '$lib/shell/ShortcutsDialog.svelte';
	import Sidebar from '$lib/shell/Sidebar.svelte';
	import AgentGutter from '$lib/features/agents/AgentGutter.svelte';
	import SessionGutter from '$lib/features/actors/SessionGutter.svelte';
	import ConversationGutter from '$lib/shell/ConversationGutter.svelte';
	import WorkingMark from '$lib/shell/WorkingMark.svelte';

	import ConfirmDialog from '$lib/ui/ConfirmDialog.svelte';
	import type { ActorSession } from '$lib/protocol/types';
	import { isGoing } from '$lib/stores/actors.svelte';
	import { connectors } from '$lib/stores/connectors.svelte';
	import { refreshAll, refreshInvalidated } from '$lib/stores/resource.svelte';
	import { workspaces } from '$lib/stores/workspaces.svelte';
	import Toast from '$lib/ui/Toast.svelte';
	import { UNREACHABLE_MESSAGE, toasts } from '$lib/ui/toasts.svelte';
	import { notify, openNotificationsWith } from '$lib/ui/notify';
	import { planOf } from '$lib/features/actors/plan';
	import { pageStores } from '$lib/stores/pageStores.svelte';

	const { actors, agents, resources } = pageStores();
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

	const routeWorkspace = $derived(page.params.ws ? Number(page.params.ws) : null);
	const routeAgent = $derived(page.params.agent ? Number(page.params.agent) : null);
	const routeSession = $derived(page.params.session ? Number(page.params.session) : null);
	/** The agent whose conversation is open, which the column beside the page speaks for. */
	const gutterAgent = $derived(agents.list.value.find((agent) => agent.id === routeAgent) ?? null);
	/** The actor session open, which the column speaks for in the same way. */
	const gutterSession = $derived(
		routeSession !== null && actors.session.key === routeSession ? actors.session.value : null
	);

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

	// The sidebar shows the current workspace's kinds of resource, in the
	// order the user put them, while a connector of theirs can import.
	$effect(() => {
		if (workspaces.currentId !== null) void resources.shownKinds.load(workspaces.currentId);
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
		agents.onNewMessage = (agent) =>
			notify({
				message: `${agent.agent_name} sent you a message.`,
				href: routes.agent(agent.id),
				avatar: { name: agent.agent_name, color: agent.agent_color, emoji: agent.agent_emoji }
			});
		void agents.loadList();
		void actors.load();
		void connectors.load();
		send('about')
			.then((about) => (hostname = about.hostname))
			.catch(() => undefined);

		const onFocus = () => void refreshAll();
		const onKey = (event: KeyboardEvent) => {
			if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
				event.preventDefault();
				paletteOpen = !paletteOpen;
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
		<Sidebar
			pathname={page.url.pathname}
			workspaceId={workspaces.currentId}
			status={connection.status}
			{hostname}
			agents={agents.list.value}
			teams={agents.teams.value}
			actorSessions={actors.current}
			goingActorSessions={actors.goingCount}
			kinds={shownKinds}
			onresolve={resolveSession}
			onsearch={() => (paletteOpen = true)}
			onchoosekinds={() => (choosingKinds = true)}
			onpin={(id, pinned) => agents.setPinned(id, pinned).catch((e) => toasts.error(e))}
			onreorder={(ids) => agents.reorder(ids).catch((e) => toasts.error(e))}
			onreorderkinds={(ids) =>
				workspaces.currentId !== null &&
				resources.reorderShown(workspaces.currentId, ids).catch((e) => toasts.error(e))}
		/>
		<!-- The page is a sheet inset in the window, the sidebar on the canvas around it. It reserves
		     no scrollbar gutter, so a bar across its top (a conversation's header) meets both edges
		     evenly; where scrollbars take room, a page that starts to scroll narrows by one.
		     The top margin keeps pages clear of a WebUI harness's window controls or status bar without scrolling
		     under them; on a phone-width screen the sidebar is a bar above the page and clears them instead.
		     It holds one half (`PaneHost`), or two side by side in a split view, each its own scroller,
		     positioned, so what a page positions absolutely (an `sr-only` label in each of a thousand
		     rows) scrolls in there rather than stretching the document into another scroller.
		     Each scroller is the outermost scope its half's navigations cross-fade (`shell/fade.ts`);
		     the sheet that draws the card sits outside them, so the card stays put while only what is
		     on it dissolves. -->
		<div
			class="m-(--sheet-inset) mt-[max(var(--sheet-inset),var(--titlebar-inset))] ml-0 flex min-h-0 min-w-0 flex-1 rounded-(--sheet-radius) bg-bg shadow-sheet max-sm:m-0 max-sm:shadow-none"
		>
			<PaneHost side="main" pane={mainPane} bind:root={mainRoot}>
				{@render children()}
			</PaneHost>
			{#if panes.split}
				<SplitView split={panes.split} onsearch={() => (paletteOpen = true)} />
			{/if}
		</div>
		<ConversationGutter open={gutterAgent !== null || gutterSession !== null}>
			{#if gutterAgent}
				<AgentGutter
					agent={gutterAgent}
					cues={agents.cues.key === gutterAgent.id ? agents.cues.value : []}
					actors={actors.list.value}
					sessions={actors.followedBy(gutterAgent.id)}
					onresolve={resolveSession}
				/>
			{:else if gutterSession}
				<SessionGutter
					session={gutterSession}
					actor={actors.list.value.find((a) => a.id === gutterSession.actor_id) ?? null}
					plan={planOf(actors.usage.value, gutterSession.actor_id)}
					family={actors.family.key === gutterSession.id ? actors.family.value : []}
				/>
			{/if}
		</ConversationGutter>
	</div>
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
