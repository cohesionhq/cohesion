<!--
	The Dashboard, where a workspace opens: a greeting for the time of day,
	then a message to an agent as the page's centrepiece (write, then pick
	who it goes to, under the composer; the reply waits in the agent's
	conversation, which the toast opens), and beside the picker a Quick
	journal button, a dialog that adds a note to today's entry. Under it,
	what's new (DashboardRecap).
-->
<script lang="ts">
	import NotebookPen from '@lucide/svelte/icons/notebook-pen';
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import { onMount, untrack } from 'svelte';
	import Composer from '$lib/features/agents/Composer.svelte';
	import { toProtocolError } from '$lib/protocol/errors';
	import type { ImageUpload } from '$lib/protocol/types';
	import { routes } from '$lib/shell/routes';
	import { pageStores } from '$lib/stores/pageStores.svelte';
	import { profile } from '$lib/stores/settings/profile.svelte';
	import AgentAvatar from '$lib/ui/AgentAvatar.svelte';
	import Picker, { type PickerItem } from '$lib/ui/Picker.svelte';
	import { panes } from '$lib/shell/pane.svelte';
	import { paneChrome } from '$lib/ui/paneChrome';
	import { toasts } from '$lib/ui/toasts.svelte';
	import { capsule, capsuleOn } from './capsule';
	import DashboardRecap from './DashboardRecap.svelte';
	import { greeting } from './greeting';
	import QuickJournal from './QuickJournal.svelte';

	let { ws }: { ws: number } = $props();

	const { actors, agents } = pageStores();
	const controls = paneChrome()?.controls;

	// One line per visit: the hour and the pick are read once, as the page opens.
	const hour = new Date().getHours();
	const pick = Math.random();
	const hello = $derived(greeting(hour, profile.profile.value?.name ?? null, pick));

	onMount(() => {
		void agents.loadList();
		void actors.load();
		void profile.load();
		return () => profile.release();
	});

	// Pinned first, in their order, then the latest to talk.
	const list = $derived(
		[...agents.list.value].sort(
			(a, b) =>
				Number(b.pinned) - Number(a.pinned) ||
				(a.pinned
					? a.sort_order - b.sort_order
					: (b.last_message_at ?? 0) - (a.last_message_at ?? 0))
		)
	);
	let choosing = $state(false);
	let journaling = $state(false);
	let chosen = $state<number | null>(null);
	// The agent picked here while it lasts, else the first in the list.
	const agentId = $derived(list.find((a) => a.id === chosen)?.id ?? list[0]?.id ?? null);
	const agent = $derived(agents.agent(agentId));
	const agentItems = $derived<PickerItem[]>(
		list.map((a) => ({
			id: a.id,
			label: a.agent_name,
			checked: a.id === agentId,
			avatar: { name: a.agent_name, color: a.agent_color, emoji: a.agent_emoji }
		}))
	);

	let draft = $state('');
	let images = $state<ImageUpload[]>([]);
	let sendError = $state<string | null>(null);
	let focusKey = $state(0);

	$effect(() => {
		void agentId;
		untrack(() => focusKey++);
	});

	async function send() {
		if (agentId === null || agent === null) return;
		const text = draft.trim();
		const attached = images;
		draft = '';
		images = [];
		sendError = null;
		try {
			await agents.sendMessage(agentId, text, attached);
			// The reply lands in the conversation; the toast opens it.
			toasts.show(`Sent to ${agent.agent_name}.`, 'info', {
				href: routes.agent(agentId),
				avatar: { name: agent.agent_name, color: agent.agent_color, emoji: agent.agent_emoji }
			});
		} catch (error) {
			sendError = toProtocolError(error).message;
			draft = text;
			images = attached;
		}
	}
</script>

<div
	class="mx-auto flex w-full max-w-[calc(var(--spacing)*180+2*var(--page-inset))] flex-col px-(--page-inset) pb-10"
>
	<!-- Its own header rather than PageHeader: the greeting centred, and no
	     split view button; only a split's own "Close this half" shows. -->
	<header class="relative pt-20 pb-4">
		<h1 class="text-center text-xl font-semibold text-balance">{hello}</h1>
		{#if panes.split && controls}
			<div class="absolute top-3.5 right-0">{@render controls()}</div>
		{/if}
	</header>

	<section aria-label="Message an agent" class="flex flex-col gap-5 pt-10">
		{#if agents.list.loaded && list.length === 0}
			<p class="text-center text-sm text-fg-subtle">
				No agents yet. <a href={routes.newAgent()}>New agent</a>
			</p>
		{/if}
		{#if agent}
			<Composer
				bind:value={draft}
				bind:images
				error={sendError}
				placeholder="Give {agent.agent_name} something to do…"
				onsend={send}
				onclearerror={() => (sendError = null)}
				{focusKey}
			/>
		{/if}

		<!-- Who the message goes to: one capsule opening a picker of every
		     agent, so it holds however many there are. -->
		<div class="flex items-center justify-between gap-2">
			<div class={[list.length === 0 && 'hidden']}>
				<Picker
					bind:open={choosing}
					items={agentItems}
					onpick={(id) => (chosen = Number(id))}
					placeholder="Find an agent"
					empty="No agents yet."
				>
					{#snippet trigger(props)}
						<button
							{...props}
							class={[capsule, 'pr-2.5 pl-1', capsuleOn, 'hover:bg-bg-hover']}
							type="button"
						>
							<span class="sr-only">Agent:</span>
							{#if agent}
								<AgentAvatar
									name={agent.agent_name}
									color={agent.agent_color}
									emoji={agent.agent_emoji}
									size="xs"
								/>
								{agent.agent_name}
							{/if}
							<ChevronDown size={14} strokeWidth={2} class="text-fg-subtle" aria-hidden="true" />
						</button>
					{/snippet}
				</Picker>
			</div>
			<button
				class={[
					capsule,
					'px-3',
					'border-border text-fg-muted hover:border-border-strong hover:text-fg'
				]}
				type="button"
				onclick={() => (journaling = true)}
			>
				<NotebookPen size={14} strokeWidth={2} aria-hidden="true" />
				Quick journal
			</button>
		</div>
	</section>

	<QuickJournal {ws} bind:open={journaling} />

	<div class="flex flex-col gap-8 pt-24">
		<DashboardRecap />
	</div>
</div>
