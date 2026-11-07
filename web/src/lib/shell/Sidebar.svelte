<!--
	App navigation. ≥900px: a column with labels, 220px until the user drags
	its edge (or focuses the edge and presses ←/→; a double-click resets it). 600–899px: an icon
	rail. <600px: a top bar. Labels stay in the DOM (visually hidden) so the
	rail and bar keep accessible names. It sits on the window's canvas,
	beside the page's sheet. The workspace's name heads it, Search an icon
	beside it. Every agent has its own link, drawn with its avatar and its
	name shimmering while it works, and in the column an eye that opens beside the pin
	and blinks twice every 5 seconds, in step with every other, while it
	follows actor sessions not yet resolved, with how many:
	pinned agents first, each marked with a pin and
	shown only there whatever its team, a rule, then the rest in the
	daemon's order, grouped under their teams by title (each group folds
	away in the column, remembered on this device) and then those on no
	team. On the rail and the top bar the groups lie open. In the column, hovering an
	agent shows the pin on the rest too, and the pin pins or unpins it. Pinned agents and the
	workspace's kinds reorder by dragging, or with Alt+↑/↓. Under them, in
	the column, the actor sessions not yet resolved,
	each led by where it stands, with a Resolve button on hover (a confirmation first if it is going), holding their rows while the pointer is over them, after a
	link to start a new one; on the rail and the top bar,
	one Actors icon counting those going on.
-->
<script lang="ts">
	import BookUser from '@lucide/svelte/icons/book-user';
	import Cable from '@lucide/svelte/icons/cable';
	import CalendarDays from '@lucide/svelte/icons/calendar-days';
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import CheckSquare from '@lucide/svelte/icons/square-check';
	import Eye from '@lucide/svelte/icons/eye';
	import FileText from '@lucide/svelte/icons/file-text';
	import Archive from '@lucide/svelte/icons/archive';
	import House from '@lucide/svelte/icons/house';
	import LayoutDashboard from '@lucide/svelte/icons/layout-dashboard';
	import LoaderCircle from '@lucide/svelte/icons/loader-circle';
	import Newspaper from '@lucide/svelte/icons/newspaper';
	import NotebookPen from '@lucide/svelte/icons/notebook-pen';
	import Pin from '@lucide/svelte/icons/pin';
	import Plus from '@lucide/svelte/icons/plus';
	import Puzzle from '@lucide/svelte/icons/puzzle';
	import Search from '@lucide/svelte/icons/search';
	import SlidersHorizontal from '@lucide/svelte/icons/sliders-horizontal';
	import Settings from '@lucide/svelte/icons/settings';
	import SquareTerminal from '@lucide/svelte/icons/square-terminal';
	import type { Component, Snippet } from 'svelte';
	import { flip } from 'svelte/animate';
	import { backOut } from 'svelte/easing';
	import { fade, fly, scale } from 'svelte/transition';
	import type { ConnectionStatus } from '$lib/protocol/connection.svelte';
	import type { ActorSession, IntelligenceAgent, IntelligenceAgentTeam } from '$lib/protocol/types';
	import { vocabulary } from '$lib/protocol/vocabulary.svelte';
	import { routes } from '$lib/shell/routes';
	import type { ConnectorKind } from '$lib/stores/connectors.svelte';
	import AgentCard from '$lib/features/agents/AgentCard.svelte';
	import WorkspaceSwitcher from '$lib/shell/WorkspaceSwitcher.svelte';
	import AgentAvatar from '$lib/ui/AgentAvatar.svelte';
	import HoverCard from '$lib/ui/HoverCard.svelte';
	import IconTile from '$lib/ui/IconTile.svelte';
	import { blinkOnPageClock, motion } from '$lib/ui/motion';
	import { Reorderable } from '$lib/ui/reorder.svelte';
	import RollingText from '$lib/ui/RollingText.svelte';
	import Shimmer from '$lib/ui/Shimmer.svelte';
	import StatusIcon from '$lib/ui/StatusIcon.svelte';

	interface Props {
		pathname: string;
		workspaceId: number | null;
		status: ConnectionStatus;
		hostname: string | null;
		/** Every agent, each linked to its conversation. */
		agents: IntelligenceAgent[];
		/** The teams agents are organized into, by title. */
		teams: IntelligenceAgentTeam[];
		/** The actor sessions not yet resolved, each linked to its transcript. */
		actorSessions: ActorSession[];
		/** How many actor sessions are going on (working or needing attention), which the rail's Actors icon counts. */
		goingActorSessions: number;
		/** The kinds of resource the workspace shows, each linked to its page, in their order. */
		kinds: ConnectorKind[];
		/** Resolves an actor session: the user is done with it. */
		onresolve: (session: ActorSession) => void;
		onsearch: () => void;
		/** Opens the choice of which kinds the workspace shows. */
		onchoosekinds: () => void;
		/** Pins or unpins an agent. */
		onpin: (agentId: number, pinned: boolean) => void;
		/** Every pinned agent's id, in the order the user put them. */
		onreorder: (agentIds: number[]) => void;
		/** Every shown kind's id, in the order the user put them. */
		onreorderkinds: (kindIds: number[]) => void;
	}

	let {
		pathname,
		workspaceId,
		status,
		hostname,
		agents,
		teams,
		actorSessions,
		goingActorSessions,
		kinds,
		onresolve,
		onsearch,
		onchoosekinds,
		onpin,
		onreorder,
		onreorderkinds
	}: Props = $props();

	/** An eye opening from a slit, or shutting to one. */
	function eyeOpen(_node: Element) {
		return { duration: motion.duration, css: (t: number) => `transform: scaleY(${t})` };
	}

	const pinned = $derived(agents.filter((agent) => agent.pinned));
	const unpinned = $derived(agents.filter((agent) => !agent.pinned));
	/** Each team with unpinned agents on it, and those agents; a pinned agent shows only above. */
	const teamGroups = $derived(
		teams
			.map((team) => ({ team, members: unpinned.filter((agent) => agent.team_id === team.id) }))
			.filter(({ members }) => members.length > 0)
	);
	const unteamed = $derived(
		unpinned.filter((agent) => !teams.some((team) => team.id === agent.team_id))
	);

	// ---- Folded teams -------------------------------------------------------

	const FOLDED_KEY = 'cohesion.sidebarFoldedTeams';

	function rememberedFolded(): number[] {
		try {
			const stored: unknown = JSON.parse(localStorage.getItem(FOLDED_KEY) ?? '[]');
			return Array.isArray(stored) ? stored.filter((id) => typeof id === 'number') : [];
		} catch {
			return [];
		}
	}

	let folded = $state(rememberedFolded());

	function toggleFolded(team_id: number) {
		folded = folded.includes(team_id)
			? folded.filter((id) => id !== team_id)
			: [...folded, team_id];
		try {
			localStorage.setItem(FOLDED_KEY, JSON.stringify(folded));
		} catch {
			// Remembering what's folded is a convenience; ignore blocked storage.
		}
	}

	const agentOrder = new Reorderable(
		() => pinned,
		(agent) => agent.id,
		(ids) => onreorder(ids)
	);
	const kindOrder = new Reorderable(
		() => kinds,
		({ kind }) => kind.id,
		(ids) => onreorderkinds(ids)
	);

	interface Link {
		href: string;
		label: string;
		icon: Component;
		/** Marks the link with a dot, e.g. for unread messages. */
		unread?: boolean;
		/** Marks the link with a spinner and this count, when there is any, e.g. for actor sessions at work. */
		running?: number;
	}

	const workspaceLinks = $derived.by((): Link[] => {
		if (workspaceId === null) return [];
		return [
			{ href: routes.dashboard(workspaceId), label: 'Dashboard', icon: LayoutDashboard },
			{ href: routes.today(workspaceId), label: 'Today', icon: CalendarDays },
			{ href: routes.journal(workspaceId), label: 'Journal', icon: NotebookPen },
			{ href: routes.documents(workspaceId), label: 'Documents', icon: FileText },
			{ href: routes.tasks(workspaceId), label: 'Tasks', icon: CheckSquare },
			{ href: routes.articles(workspaceId), label: 'Articles', icon: Newspaper },
			{ href: routes.contacts(workspaceId), label: 'Contacts', icon: BookUser }
		];
	});

	// While the pointer is over the actor sessions they hold their rows, so one read, finished
	// or started moves only once it leaves and a row never shifts under a click. Each held
	// row still shows where its session stands, and goes the moment the sidebar stops listing it
	// (a session resolved from here leaves at once, pointer or not).
	let heldSessions = $state.raw<ActorSession[] | null>(null);
	const shownTasks = $derived(
		heldSessions?.flatMap(
			(held) => actorSessions.find((session) => session.id === held.id) ?? []
		) ?? actorSessions
	);

	// The agent whose hover card shows; one at a time, like a browser's tabs.
	let cardFor = $state<number | null>(null);

	// ---- Width --------------------------------------------------------------

	const WIDTH_KEY = 'cohesion.sidebarWidth';
	const DEFAULT_WIDTH = 220;
	const MIN_WIDTH = 180;
	const MAX_WIDTH = 400;

	const clampWidth = (w: number) => Math.round(Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, w)));

	function rememberedWidth(): number {
		try {
			const stored = Number(localStorage.getItem(WIDTH_KEY));
			return stored ? clampWidth(stored) : DEFAULT_WIDTH;
		} catch {
			return DEFAULT_WIDTH;
		}
	}

	let width = $state(rememberedWidth());
	let resizing = $state(false);
	let navEl = $state<HTMLElement>();

	function setWidth(next: number) {
		width = clampWidth(next);
		try {
			localStorage.setItem(WIDTH_KEY, String(width));
		} catch {
			// Remembering the width is a convenience; ignore blocked storage.
		}
	}

	function onResizeStart(event: PointerEvent) {
		if (event.button !== 0) return;
		event.preventDefault();
		(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
		resizing = true;
	}

	function onResizeMove(event: PointerEvent) {
		if (resizing && navEl) setWidth(event.clientX - navEl.getBoundingClientRect().left);
	}

	function onResizeKey(event: KeyboardEvent) {
		const step = event.shiftKey ? 48 : 16;
		if (event.key === 'ArrowLeft') setWidth(width - step);
		else if (event.key === 'ArrowRight') setWidth(width + step);
		else if (event.key === 'Home') setWidth(MIN_WIDTH);
		else if (event.key === 'End') setWidth(MAX_WIDTH);
		else return;
		event.preventDefault();
	}

	function active(href: string): boolean {
		return pathname === href || pathname.startsWith(`${href}/`);
	}

	const linkClass =
		'relative flex h-7.5 cursor-pointer items-center gap-2.5 rounded-sm px-2 text-left no-underline hover:bg-bg-hover hover:text-fg hover:no-underline max-md:w-9 max-md:justify-center max-md:p-0 [&_svg]:flex-none';

	/** The current link takes a neutral fill and full-strength text and icon. */
	const linkState = (current: boolean) =>
		current
			? 'bg-current-item font-medium text-fg [&>svg]:text-fg'
			: 'text-fg-muted [&>svg]:text-fg-subtle';

	/** Visually hidden on the rail and the top bar, where only icons show. */
	const labelClass =
		'max-md:absolute max-md:h-px max-md:w-px max-md:overflow-hidden max-md:[clip:rect(0_0_0_0)] max-md:whitespace-nowrap';

	const statusLabel = $derived(
		status === 'connected'
			? hostname
				? `Connected to ${hostname}`
				: 'Connected'
			: status === 'connecting'
				? 'Connecting…'
				: 'Reconnecting…'
	);
</script>

{#snippet navLink(link: Link)}
	{@const Icon = link.icon}
	{@const current = active(link.href)}
	<a
		class={[linkClass, linkState(current)]}
		href={link.href}
		aria-current={current ? 'page' : undefined}
	>
		<Icon size={16} strokeWidth={1.75} />
		<span class={labelClass}>{link.label}</span>
		{#if link.unread}{@render unreadDot()}{/if}
		{#if link.running}{@render runningCount(link.running)}{/if}
	</a>
{/snippet}

{#snippet kindLink(ws: number, entry: ConnectorKind, index: number)}
	{@const { connector, kind } = entry}
	{@const href = routes.resources(ws, kind.id)}
	{@const current = active(href)}
	<a
		class={[linkClass, linkState(current), kindOrder.dragged === kind.id && 'opacity-50']}
		{href}
		aria-current={current ? 'page' : undefined}
		aria-keyshortcuts="Alt+ArrowUp Alt+ArrowDown"
		draggable="true"
		ondragstart={(e) => kindOrder.start(e, entry, kind.plural)}
		{...kindOrder.target(index)}
		ondragend={() => kindOrder.end()}
		onkeydown={(e) => kindOrder.key(e, index)}
	>
		<IconTile icon={connector.icon} title={connector.title} size={16} />
		<span class={['truncate', labelClass]}>{kind.plural}</span>
	</a>
{/snippet}

{#snippet unreadDot()}
	<span class="sr-only">, unread messages</span>
	<!-- After the label in the column; on the icon's corner on the rail and the top bar. -->
	<span
		class="ml-auto size-1.5 flex-none rounded-full bg-accent max-md:absolute max-md:top-1.5 max-md:right-1.5"
		aria-hidden="true"
	></span>
{/snippet}

{#snippet followingCount(count: number)}
	{#if count > 0}
		<span class="sr-only">, following {count} actor {count === 1 ? 'session' : 'sessions'}</span>
		<!-- In the column only: the rail and the top bar have no corner left for it. -->
		<span
			class="flex flex-none items-center gap-1 text-xs text-fg-subtle tabular-nums max-md:hidden"
			aria-hidden="true"
			transition:fade={{ duration: motion.duration }}
		>
			<span class="flex" transition:eyeOpen>
				<span class="flex" {@attach blinkOnPageClock}><Eye size={12} strokeWidth={2} /></span>
			</span>
			<RollingText text={String(count)} />
		</span>
	{/if}
{/snippet}

{#snippet runningCount(count: number)}
	<span class="sr-only">, {count} running</span>
	<!-- After the label in the column; on the icon's corner on the rail and the top bar. -->
	<span
		class="ml-auto flex flex-none items-center gap-1 text-xs text-fg-subtle tabular-nums max-md:absolute max-md:top-1 max-md:right-1"
		aria-hidden="true"
		transition:fade={{ duration: motion.duration }}
	>
		<LoaderCircle
			class="text-accent motion-safe:animate-spin max-md:size-2.5"
			size={12}
			strokeWidth={2}
		/>
		<span class="max-md:hidden">{count}</span>
	</span>
{/snippet}

{#snippet agentCard(agent: IntelligenceAgent, link: Snippet<[Record<string, unknown>]>)}
	<!-- Resting on an agent shows its card, beside the column or below the top bar. -->
	<HoverCard
		bind:open={
			() => cardFor === agent.id,
			(open) => {
				if (open) cardFor = agent.id;
				else if (cardFor === agent.id) cardFor = null;
			}
		}
		openDelay={cardFor === null ? 500 : 0}
		disabled={agentOrder.dragged !== null}
	>
		{#snippet trigger(props)}{@render link(props)}{/snippet}
		<AgentCard {agent} />
	</HoverCard>
{/snippet}

{#snippet agentLink(agent: IntelligenceAgent, pinnedAt?: number)}
	{@const href = routes.agent(agent.id)}
	{@const current = active(href)}
	{@const isPinned = pinnedAt !== undefined}
	{@const order = isPinned
		? {
				draggable: 'true',
				ondragstart: (e: DragEvent) => agentOrder.start(e, agent, agent.agent_name),
				...agentOrder.target(pinnedAt),
				ondragend: () => agentOrder.end()
			}
		: {}}
	{@const keys = isPinned
		? {
				'aria-keyshortcuts': 'Alt+ArrowUp Alt+ArrowDown',
				onkeydown: (e: KeyboardEvent) => agentOrder.key(e, pinnedAt)
			}
		: {}}
	{#snippet link(props: Record<string, unknown>)}
		<a
			{...props}
			{...keys}
			class={[linkClass, linkState(current)]}
			{href}
			draggable="false"
			aria-current={current ? 'page' : undefined}
		>
			<!-- A 20px avatar in the 16px icon slot, pulled in so labels still line up. -->
			<span class="relative -mx-0.5 flex">
				<AgentAvatar
					name={agent.agent_name}
					color={agent.agent_color}
					emoji={agent.agent_emoji}
					size="xs"
				/>
				<!-- The rail and the top bar hide the name that shows the agent working, so a ring on the icon does. -->
				{#if agent.activity !== null}
					<span
						class="working-ring absolute -inset-0.75 hidden rounded-full max-md:block"
						aria-hidden="true"
						transition:fade={{ duration: motion.duration }}
					></span>
				{/if}
			</span>
			<Shimmer class={['truncate', labelClass]} active={agent.activity !== null}
				>{agent.agent_name}</Shimmer
			>
			<!-- After the label in the column; on the icon's corner on the rail and the top bar.
			     Always drawn, so what is in it can fade in and out. -->
			<span class="ml-auto flex flex-none items-center gap-1.5 max-md:contents">
				{#if agent.activity !== null}<span class="sr-only">, working</span>{/if}
				{#if agent.agent_unread}{@render unreadDot()}{/if}
				{@render followingCount(agent.following_count)}
				{#if isPinned}<span class="sr-only">, pinned</span>{/if}
				<!-- Room for the pin button, which sits over it: always for a pinned agent, on hover for the rest. -->
				<span
					class={[
						'size-3 flex-none max-md:hidden',
						!isPinned && 'hidden group-focus-within/agent:block group-hover/agent:block'
					]}
					aria-hidden="true"
				></span>
			</span>
		</a>
	{/snippet}
	<div class={['group/agent relative', agentOrder.dragged === agent.id && 'opacity-50']} {...order}>
		{@render agentCard(agent, link)}
		<!-- Beside the link, not in it, so it stays a button of its own; centred on the room the link leaves. -->
		<button
			class={[
				'absolute top-1/2 right-1 flex size-5 -translate-y-1/2 cursor-pointer items-center justify-center rounded-sm text-fg-subtle hover:text-fg max-md:hidden',
				!isPinned && 'opacity-0 group-focus-within/agent:opacity-100 group-hover/agent:opacity-100'
			]}
			type="button"
			aria-label="{isPinned ? 'Unpin' : 'Pin'} {agent.agent_name}"
			onclick={() => onpin(agent.id, !isPinned)}
		>
			<Pin size={12} strokeWidth={1.75} aria-hidden="true" />
		</button>
	</div>
{/snippet}

<nav
	bind:this={navEl}
	style:--nav-width="{width}px"
	class="flex h-full w-(--nav-width) flex-none [scrollbar-width:none] flex-col gap-3 overflow-hidden px-2.5 pt-[calc(--spacing(3)+var(--titlebar-inset))] pb-3 max-md:w-(--rail-width) max-md:items-center max-md:px-1 max-sm:h-auto max-sm:w-full max-sm:flex-row max-sm:gap-1 max-sm:overflow-x-auto max-sm:border-b max-sm:border-border max-sm:px-2 max-sm:pt-[calc(--spacing(1)+var(--titlebar-inset))] max-sm:pb-1"
	aria-label="Main"
>
	<!-- The workspace's name heads the column, Search an icon at the end of its row (⌘K opens it too).
	     On the rail they stack; on the top bar they take their places in the row. -->
	<div class="flex flex-none items-center gap-1 max-md:flex-col max-sm:contents">
		<div class="min-w-0 flex-1 max-md:flex-none"><WorkspaceSwitcher /></div>
		<button
			class={[
				linkClass,
				'flex-none text-fg-subtle md:w-7.5 md:justify-center md:px-0 max-sm:order-1'
			]}
			type="button"
			aria-label="Search"
			onclick={onsearch}
		>
			<Search size={16} strokeWidth={1.75} />
		</button>
	</div>

	<!-- Only what lies between Search and the foot scrolls; those two hold their place. On the top bar it all scrolls as one row. -->
	<div
		class="-mx-2.5 flex min-h-0 flex-1 [scrollbar-width:none] flex-col gap-4 overflow-y-auto px-2.5 max-md:-mx-1 max-md:items-center max-md:px-1 max-sm:contents"
	>
		<!-- The kinds a workspace shows are workspace content too, a little apart from the rest. -->
		<div class="flex flex-col gap-2 max-sm:flex-row max-sm:gap-px">
			<div class="flex flex-col gap-px max-sm:flex-row">
				{#each workspaceLinks as link (link.label)}
					{@render navLink(link)}
				{/each}
			</div>
			{#if workspaceId !== null}
				<div class="flex flex-col gap-px max-sm:flex-row" {...kindOrder.list}>
					{#each kindOrder.items as entry, index (entry.kind.id)}
						{@render kindLink(workspaceId, entry, index)}
					{/each}
					<button class={[linkClass, 'text-fg-muted']} type="button" onclick={onchoosekinds}>
						<SlidersHorizontal size={16} strokeWidth={1.75} />
						<span class={labelClass}>Edit…</span>
					</button>
				</div>
			{/if}
		</div>

		<div class="flex flex-col gap-px max-sm:flex-row" role="group" aria-labelledby="nav-agents">
			<a
				class={[
					'px-2 pb-1 text-xs font-medium text-fg-subtle no-underline hover:text-fg hover:no-underline',
					labelClass
				]}
				href={routes.agents()}
				id="nav-agents">Agents</a
			>
			{#if pinned.length}
				<!-- First, in the order the user put them, then a rule before the rest. -->
				<div class="flex flex-col gap-px max-sm:flex-row" {...agentOrder.list}>
					{#each agentOrder.items as agent, index (agent.id)}
						{@render agentLink(agent, index)}
					{/each}
				</div>
				{#if unpinned.length}
					<div class="mx-2 my-1 h-px flex-none bg-border max-md:mx-1 max-sm:hidden"></div>
				{/if}
			{/if}
			{#each teamGroups as { team, members }, index (team.id)}
				{@const open = !folded.includes(team.id)}
				<!-- The column folds a team away; the rail and the top bar have no room for its name, so it lies open there. -->
				<button
					class={[
						'flex cursor-pointer items-center gap-1 px-2 pb-0.5 text-xs text-fg-subtle hover:text-fg max-md:hidden',
						// Under the rule after the pinned agents, its own margin is the gap.
						index === 0 && pinned.length && unpinned.length ? 'pt-0.5' : 'pt-1.5'
					]}
					type="button"
					aria-expanded={open}
					aria-controls="nav-team-{team.id}"
					onclick={() => toggleFolded(team.id)}
				>
					<ChevronRight
						class={['flex-none transition-transform', open && 'rotate-90']}
						size={12}
						strokeWidth={1.75}
						aria-hidden="true"
					/>
					<span class="truncate">{team.title}</span>
				</button>
				<div
					class={['flex flex-col gap-px max-sm:flex-row', !open && 'hidden max-md:flex']}
					id="nav-team-{team.id}"
					role="group"
					aria-label={team.title}
				>
					{#each members as agent (agent.id)}
						{@render agentLink(agent)}
					{/each}
				</div>
			{/each}
			{#if teamGroups.length && unteamed.length}
				<div class="mx-2 my-1 h-px flex-none bg-border max-md:mx-1 max-sm:hidden"></div>
			{/if}
			{#each unteamed as agent (agent.id)}
				{@render agentLink(agent)}
			{/each}
			<!-- An action, not an agent: set a little apart and quieter. -->
			<div class="mt-1 flex flex-col max-sm:mt-0 [&>a]:text-fg-subtle">
				{@render navLink({ href: routes.newAgent(), label: 'New agent', icon: Plus })}
			</div>
		</div>

		<!-- Titles alone don't fit the rail or the top bar, which keep one Actors icon instead. -->
		<div
			class="flex flex-col gap-px max-md:hidden"
			role="group"
			aria-labelledby="nav-actors"
			onpointerenter={() => (heldSessions = actorSessions)}
			onpointerleave={() => (heldSessions = null)}
		>
			<a
				class="px-2 pb-1 text-xs font-medium text-fg-subtle no-underline hover:text-fg hover:no-underline"
				href={routes.actors()}
				id="nav-actors">Actors</a
			>
			<!-- Current only on the Actors page itself, not on a session's under it. -->
			<a
				class={[linkClass, linkState(pathname === routes.actors())]}
				href={routes.actors()}
				aria-current={pathname === routes.actors() ? 'page' : undefined}
			>
				<House size={16} strokeWidth={1.75} />
				<span>Home</span>
			</a>
			<!-- A session arriving flies in, one leaving fades where it was, and the rest flip into
			     their new places; its icon pops when its status changes. `animate:` needs the
			     row as the block's direct child, so the link is written out here. -->
			{#each shownTasks as session (session.id)}
				{@const href = routes.actorSession(session.id)}
				{@const current = active(href)}
				<div
					class="group/session relative flex flex-col"
					in:fly={{ y: 8, duration: motion.duration }}
					out:fade={{ duration: motion.duration }}
					animate:flip={{ duration: motion.duration }}
				>
					<!-- The row's right edge clears the Resolve button whenever that shows. -->
					<a
						class={[
							linkClass,
							linkState(current),
							'group-focus-within/session:pr-8 group-hover/session:pr-8 touch:pr-8'
						]}
						{href}
						aria-current={current ? 'page' : undefined}
					>
						<!-- Where it stands, where the other rows draw their icon; a done session reads as ready until seen. -->
						<span class="sr-only">{vocabulary.actorSessionStatus(session.status)}, </span>
						<span class="flex w-4 flex-none justify-center">
							{#key session.status}
								<span
									class="flex"
									in:scale={{ start: 0.4, duration: motion.duration, easing: backOut }}
									><StatusIcon status={session.status} unread={session.unread} size={14} /></span
								>
							{/key}
						</span>
						<span class="truncate">{session.title}</span>
						{#if session.unread && session.status !== 'idle'}
							<span class="sr-only">, unread</span>
							<span
								class="ml-auto size-1.5 flex-none rounded-full bg-accent group-focus-within/session:hidden group-hover/session:hidden touch:hidden"
								aria-hidden="true"
								transition:scale={{ duration: motion.duration }}
							></span>
						{/if}
					</a>
					<!-- Beside the link, not in it: hover or focus shows it, a touch screen always does. -->
					<button
						type="button"
						class="absolute top-1/2 right-1 flex size-6 -translate-y-1/2 cursor-pointer items-center justify-center rounded-md border-0 bg-transparent text-fg-subtle opacity-0 group-focus-within/session:opacity-100 group-hover/session:opacity-100 hover:bg-bg-hover hover:text-fg touch:opacity-100"
						aria-label="Resolve “{session.title}”"
						onclick={() => onresolve(session)}
					>
						<Archive size={14} strokeWidth={1.75} />
					</button>
				</div>
			{/each}
		</div>
		<div class="flex flex-col gap-px md:hidden max-sm:flex-row">
			{@render navLink({
				href: routes.actors(),
				label: 'Actors',
				icon: SquareTerminal,
				running: goingActorSessions
			})}
		</div>
	</div>

	<!-- Configuration sits apart, at the foot, under a rule that ends what scrolls above it. -->
	<div class="flex flex-none flex-col gap-px max-sm:order-2 max-sm:ml-auto max-sm:flex-row">
		<div class="mx-2 mb-2 h-px flex-none bg-border max-md:mx-1 max-sm:hidden"></div>
		{@render navLink({ href: routes.connectorAccounts(), label: 'Accounts', icon: Cable })}
		{@render navLink({ href: routes.extensions(), label: 'Extensions', icon: Puzzle })}
		<div
			class="flex items-center justify-between max-md:flex-col max-md:gap-2 max-sm:flex-row [&>a]:flex-1 max-md:[&>a]:flex-none"
		>
			{@render navLink({ href: routes.settings(), label: 'Settings', icon: Settings })}
			<!-- Grey when connected, so it never reads as an unread dot. -->
			<span class="inline-grid size-6 place-items-center" aria-label={statusLabel} role="img">
				<span
					class={[
						'size-[7px] rounded-full',
						status === 'connected' && 'bg-fg-subtle',
						status === 'connecting' && 'border border-fg-subtle',
						status === 'reconnecting' && 'border border-warn'
					]}
				></span>
			</span>
		</div>
	</div>
</nav>

<!-- The column's resize handle: a strip over its right border, taking no room of its own. -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions (a focusable separator is the ARIA window-splitter pattern) -->
<div
	class={[
		'relative z-10 -mx-1 w-2 flex-none cursor-col-resize touch-none outline-none after:absolute after:inset-y-0 after:left-[3px] after:w-0.5 after:transition-colors hover:after:bg-border-strong focus-visible:after:bg-accent max-md:hidden',
		resizing && 'after:bg-accent'
	]}
	role="separator"
	aria-orientation="vertical"
	aria-label="Resize sidebar"
	aria-valuenow={width}
	aria-valuemin={MIN_WIDTH}
	aria-valuemax={MAX_WIDTH}
	tabindex="0"
	onpointerdown={onResizeStart}
	onpointermove={onResizeMove}
	onpointerup={() => (resizing = false)}
	onpointercancel={() => (resizing = false)}
	ondblclick={() => setWidth(DEFAULT_WIDTH)}
	onkeydown={onResizeKey}
></div>

<style>
	/* A closed ring, still, under reduced motion; otherwise an arc of it turning. */
	.working-ring {
		border: 1.5px solid var(--fg-subtle);
	}

	@media (prefers-reduced-motion: no-preference) {
		.working-ring {
			border-color: var(--fg-subtle) transparent transparent;
			animation: working-ring-turn 2s linear infinite;
		}
	}

	@keyframes working-ring-turn {
		to {
			rotate: 360deg;
		}
	}
</style>
