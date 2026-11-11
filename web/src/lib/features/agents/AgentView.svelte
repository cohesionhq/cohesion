<!--
	The Agents page: the selected agent's conversation with its status,
	cues and a composer. The main sidebar lists the agents; with none
	selected this shows them too, or how to make one. `?new` opens the
	new-agent dialog.
-->
<script lang="ts">
	import Bot from '@lucide/svelte/icons/bot';
	import { onMount, tick, untrack } from 'svelte';
	import { toProtocolError } from '$lib/protocol/errors';
	import type { ImageUpload, AgentChoice, AgentSkill } from '$lib/protocol/types';
	import { routes } from '$lib/shell/routes';
	import type { AgentMessage } from '$lib/stores/agents.svelte';
	import { rememberText, rememberedText } from '$lib/stores/drafts';
	import { spend } from '$lib/stores/settings/spend.svelte';
	import { skills, skillsOf } from '$lib/stores/skills.svelte';
	import Button from '$lib/ui/Button.svelte';
	import EmptyState from '$lib/ui/EmptyState.svelte';
	import PageHeader from '$lib/ui/PageHeader.svelte';
	import ResourceState from '$lib/ui/ResourceState.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';
	import WorkingLine from '$lib/ui/WorkingLine.svelte';
	import AgentDialog from './AgentDialog.svelte';
	import AgentDirectory from './AgentDirectory.svelte';
	import AgentFeed from './AgentFeed.svelte';
	import AgentHeader from './AgentHeader.svelte';
	import AgentMenu from './AgentMenu.svelte';
	import Composer from './Composer.svelte';
	import CuesDialog from './CuesDialog.svelte';
	import MemoriesDialog from './MemoriesDialog.svelte';
	import RunsDialog from './RunsDialog.svelte';
	import SkillsDialog from './SkillsDialog.svelte';
	import TeamDialog from './TeamDialog.svelte';
	import { pane } from '$lib/shell/pane.svelte';
	import PageTitle from '$lib/shell/PageTitle.svelte';
	import { pageStores } from '$lib/stores/pageStores.svelte';

	const { agents } = pageStores();

	const page = pane();

	interface Props {
		/** The route's agent, or null for the list. */
		agentId: number | null;
	}

	let { agentId }: Props = $props();

	let draft = $state('');
	/** Where this browser keeps the agent's unsent message, set with `draft` so neither runs ahead. */
	let draftKey = $state<string | null>(null);
	let images = $state<ImageUpload[]>([]);
	/** The skill the next message runs, picked with `/` in the composer. */
	let skill = $state<AgentSkill | null>(null);
	/** Why the last message sent from here failed; cleared by the next one. */
	let sendError = $state<string | null>(null);
	let focusKey = $state(0);
	let creating = $state(false);
	let editing = $state(false);
	let runsOpen = $state(false);
	let cuesOpen = $state(false);
	let memoriesOpen = $state(false);
	let skillsOpen = $state(false);
	let newTeamOpen = $state(false);
	/** The agent "New team…" in its menu moves onto the team once it's made. */
	let joining = $state<number | null>(null);
	let scroller = $state<HTMLElement>();
	/** The floating composer's height, which the feed leaves clear at its foot. */
	let composerHeight = $state(0);

	$effect(() => {
		const id = agentId;
		untrack(() => {
			void agents.open(id);
			draftKey = id === null ? null : `agent.${id}`;
			draft = draftKey === null ? '' : rememberedText(draftKey);
			images = [];
			skill = null;
			sendError = null;
			focusKey++;
		});
	});

	$effect(() => {
		if (draftKey !== null) rememberText(draftKey, draft);
	});

	// The sidebar's "New agent" lands here with `?new`.
	$effect(() => {
		if (!page.url.searchParams.has('new')) return;
		untrack(() => {
			creating = true;
			const url = new URL(page.url);
			url.searchParams.delete('new');
			void page.goto(`${url.pathname}${url.search}`, { replaceState: true, keepFocus: true });
		});
	});

	onMount(() => {
		void agents.loadList();
		// Read for the composer's note while a spend limit holds jobs and forks back.
		void spend.load();
		// The skills the composer offers on `/`.
		void skills.load();
		// Deselect too, so a message from the last agent shown toasts elsewhere.
		return () => {
			void agents.open(null);
			spend.periods.release();
			skills.list.release();
		};
	});

	const agent = $derived(agents.agent(agentId));
	const agentSkills = $derived(agent ? skillsOf(skills.list.value, agent) : []);

	// An open conversation is a read one, including messages that arrive while it's shown.
	$effect(() => {
		if (agent?.agent_unread) agents.markRead(agent.id).catch((error) => toasts.error(error));
	});
	const feed = $derived(agentId !== null && agents.feed.key === agentId ? agents.feed.value : null);
	const cues = $derived(agentId !== null && agents.cues.key === agentId ? agents.cues.value : null);
	const outbox = $derived(agentId === null ? [] : (agents.outbox[agentId] ?? []));
	// Fetch the images of the messages shown that have some.
	$effect(() => {
		for (const message of feed?.messages ?? [])
			if (message.attachment_count > 0 && !agents.attachments[message.id])
				untrack(() => agents.loadAttachments(message)).catch(() => {});
	});
	const missing = $derived(
		agentId !== null &&
			((agents.list.loaded && agent === null) || agents.feed.error?.code === 'notFound')
	);

	// The feed opens at its newest message and follows new ones while the
	// reader is at (or near) the bottom; scrolled up, it leaves them be.
	let content = $state<HTMLElement>();
	let following = true;
	let lastTop = 0;

	function nearBottom(el: HTMLElement): boolean {
		return el.scrollHeight - el.scrollTop - el.clientHeight < 48;
	}

	function toBottom() {
		if (scroller) scroller.scrollTop = scroller.scrollHeight;
	}

	// Only scrolling up lets go of the bottom. A scroll event lands after
	// the frame that caused it, so by then the feed may have grown past a
	// jump to the bottom (the history mounting, a reply rendering); judged
	// on distance alone, that would read as the reader leaving and strand
	// the feed partway up.
	function onscroll() {
		if (!scroller) return;
		const top = scroller.scrollTop;
		if (nearBottom(scroller)) following = true;
		else if (top < lastTop) following = false;
		lastTop = top;
	}

	$effect(() => {
		void agentId;
		following = true;
		lastTop = 0;
		untrack(toBottom);
	});

	// Whatever grows the feed (a message, a reply, the working line, a
	// refetch) or shrinks the viewport keeps a following reader at the bottom.
	$effect(() => {
		if (!scroller || !content) return;
		const observer = new ResizeObserver(() => {
			if (following) toBottom();
		});
		observer.observe(scroller);
		observer.observe(content);
		return () => observer.disconnect();
	});

	async function loadOlder() {
		const before = scroller?.scrollHeight ?? 0;
		try {
			await agents.loadOlder();
		} catch (error) {
			toasts.error(error);
			return;
		}
		await tick();
		if (scroller) scroller.scrollTop += scroller.scrollHeight - before;
	}

	async function send() {
		if (agentId === null) return;
		const text = draft.trim();
		const attached = images;
		const using = skill;
		draft = '';
		images = [];
		skill = null;
		sendError = null;
		// Sending is a request to see the conversation's end.
		following = true;
		try {
			await agents.sendMessage(agentId, text, attached, undefined, using ?? undefined);
		} catch (error) {
			// Shown under the composer; give the message back so it can be
			// resent. Those queued behind it fail too, and follow it in order.
			sendError = toProtocolError(error).message;
			draft = draft.trim() === '' ? text : text === '' ? draft : `${draft}\n\n${text}`;
			images = [...images, ...attached];
			skill ??= using;
		}
	}

	function stop() {
		if (agentId === null) return;
		agents.stopTurn(agentId).catch((error) => toasts.error(error));
	}

	async function react(message: AgentMessage, reaction: string | null) {
		try {
			await agents.react(message, reaction);
		} catch (error) {
			toasts.error(error);
		}
	}

	function pick(message: AgentMessage, choice: AgentChoice, option: number) {
		// Picking is a request to see the agent's answer.
		following = true;
		return agents.pick(message, choice, option);
	}

	async function remove() {
		if (agentId === null) return;
		await agents.delete(agentId);
		await page.goto(routes.agents(), { replaceState: true });
	}

	function newAgent() {
		creating = true;
	}
</script>

<PageTitle title={`${agent ? `${agent.agent_name} · Agents` : 'Agents'}`} />

<!-- Fill the content area (over its bottom padding) and scroll inside, so
     the composer stays put. -->
<!-- In an agent's conversation its main actions (send, approve) take the agent's color. -->
<div
	class="-mb-12 flex h-[calc(100%+var(--s8))]"
	style:--primary={agent ? `var(--agent-${agent.agent_color})` : undefined}
	style:--primary-hover={agent
		? `color-mix(in oklch, var(--agent-${agent.agent_color}) 85%, var(--fg))`
		: undefined}
	style:--primary-fg={agent ? 'var(--on-fill)' : undefined}
>
	<div class="relative flex min-w-0 flex-1 flex-col">
		<!-- Anchoring off: loadOlder keeps the reader's place itself. No scrollbar: the feed scrolls like a chat.
		     Positioned, so what it positions absolutely is clipped by it rather than scrolling the page. -->
		<div
			class="relative min-h-0 flex-1 [scrollbar-width:none] overflow-y-auto pb-6 [overflow-anchor:none]"
			style:padding-bottom={composerHeight ? `calc(${composerHeight}px + var(--s6))` : undefined}
			bind:this={scroller}
			{onscroll}
		>
			<div class="px-(--page-inset)" bind:this={content}>
				{#if agentId === null}
					<PageHeader title="Agents">
						{#snippet actions()}
							<Button onclick={() => (newTeamOpen = true)}>New team…</Button>
							<Button onclick={newAgent}>New agent…</Button>
						{/snippet}
					</PageHeader>
					<ResourceState resource={agents.list}>
						{#snippet loaded()}
							{#if agents.list.value.length === 0}
								<div
									class="mx-auto flex min-h-[70vh] max-w-90 flex-col items-center justify-center gap-3 text-center"
								>
									<Bot class="text-fg-subtle" size={36} strokeWidth={1.25} aria-hidden="true" />
									<div>
										<p class="font-medium">No agents yet</p>
										<p class="mt-1 text-fg-muted">
											An agent keeps its own conversation, works in your workspaces, and can wake
											itself when something happens.
										</p>
									</div>
									<Button variant="primary" onclick={newAgent}>New agent…</Button>
								</div>
							{:else}
								<AgentDirectory agents={agents.list.value} teams={agents.teams.value} />
							{/if}
						{/snippet}
					</ResourceState>
				{:else if missing}
					<PageHeader title="Agents" />
					<EmptyState
						message="Agent not found"
						actionLabel="Back to agents"
						onaction={() => page.goto(routes.agents(), { replaceState: true })}
					/>
				{:else if agent}
					<!-- Pinned while the feed scrolls under it. -->
					<div class="sticky top-0 z-10">
						<AgentHeader {agent}>
							{#snippet actions()}
								<AgentMenu
									id={agent.id}
									name={agent.agent_name}
									pinned={agent.pinned}
									onpin={() =>
										agents.setPinned(agent.id, !agent.pinned).catch((error) => toasts.error(error))}
									teams={agents.teams.value}
									teamId={agent.team_id}
									onmove={(team_id) =>
										agents.setTeam(agent.id, team_id).catch((error) => toasts.error(error))}
									onnewteam={() => {
										joining = agent.id;
										newTeamOpen = true;
									}}
									onedit={() => (editing = true)}
									oncues={cues?.length ? () => (cuesOpen = true) : undefined}
									onskills={() => (skillsOpen = true)}
									onmemories={() => (memoriesOpen = true)}
									onruns={() => (runsOpen = true)}
									ondelete={remove}
								/>
							{/snippet}
						</AgentHeader>
					</div>

					{#if agents.feed.error && !agents.feed.loaded}
						<EmptyState
							message={agents.feed.error.message}
							actionLabel="Try again"
							onaction={() => agents.feed.refetch()}
						/>
					{:else if feed}
						{#if feed.hasOlder}
							<div class="mx-auto mb-4 flex max-w-190 justify-center">
								<Button onclick={loadOlder} disabled={agents.loadingOlder}>
									{agents.loadingOlder ? 'Loading…' : 'Load earlier messages'}
								</Button>
							</div>
						{/if}
						<!-- Mounted once loaded, so the history it opens with doesn't animate in. -->
						{#if agents.feed.loaded}
							<AgentFeed
								{agent}
								messages={feed.messages}
								attachments={agents.attachments}
								onreact={react}
								onpick={pick}
							/>
						{/if}
						<!-- The daemon says when the agent works: on a message, a pick, a reaction or a cue. -->
						{#if outbox.length > 0 || agent.activity !== null}
							<WorkingLine activity={agent.activity} />
						{/if}
					{/if}
				{/if}
			</div>
		</div>

		{#if agent && !missing}
			<!-- Floats over the feed's foot, which scrolls under it; the feed pads
			     its end by this height so the last message clears it. -->
			<div
				class="pointer-events-none absolute inset-x-0 bottom-0 pb-4"
				bind:clientHeight={composerHeight}
			>
				<!-- The conversation's column, which the feed reads in. -->
				<div
					class="mx-auto w-full max-w-[calc(var(--spacing)*190+2*var(--page-inset))] px-(--page-inset)"
				>
					<Composer
						bind:value={draft}
						bind:images
						bind:skill
						skills={agentSkills}
						error={sendError}
						placeholder="Message {agent.agent_name}…"
						onsend={send}
						onstop={agent.foreground_started !== null ? stop : undefined}
						onclearerror={() => (sendError = null)}
						{focusKey}
						notice={spend.reached.length > 0 ? spendNotice : undefined}
					/>
				</div>
			</div>
		{/if}
	</div>
</div>

{#snippet spendNotice()}
	A spend limit is reached, so agents call models only to answer your messages.
	<a class="pointer-events-auto" href={routes.settings('intelligence')}>Spend limits</a>
{/snippet}

<AgentDialog bind:open={creating} onsaved={(created) => page.goto(routes.agent(created.id))} />
<TeamDialog
	bind:open={newTeamOpen}
	onsaved={(team) => {
		if (joining !== null) agents.setTeam(joining, team.id).catch((error) => toasts.error(error));
	}}
/>
{#if agent}
	<AgentDialog bind:open={editing} {agent} />
	<MemoriesDialog bind:open={memoriesOpen} {agent} />
	<SkillsDialog bind:open={skillsOpen} {agent} team={agents.team(agent.team_id)} />
	<RunsDialog bind:open={runsOpen} agentId={agent.id} />
	<CuesDialog
		bind:open={cuesOpen}
		cues={cues ?? []}
		onremove={(cue_id) => agents.removeCue(cue_id)}
	/>
{/if}
