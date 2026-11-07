<!--
	The Dashboard, where a workspace opens: a greeting for the time of day as
	the page's title, then a message to an agent (who it goes to picked in the
	composer; the reply waits in the agent's conversation, which the toast
	opens). Quick journal, in the header's tools, opens a dialog that adds a
	note to today's entry. Under it, the tasks due today (DashboardToday) and
	what's new (DashboardRecap).
-->
<script lang="ts">
	import NotebookPen from '@lucide/svelte/icons/notebook-pen';
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import { onMount, untrack } from 'svelte';
	import Composer from '$lib/features/agents/Composer.svelte';
	import Button from '$lib/ui/Button.svelte';
	import { toProtocolError } from '$lib/protocol/errors';
	import type { ImageUpload } from '$lib/protocol/types';
	import { routes } from '$lib/shell/routes';
	import { pageStores } from '$lib/stores/pageStores.svelte';
	import { profile } from '$lib/stores/settings/profile.svelte';
	import AgentAvatar from '$lib/ui/AgentAvatar.svelte';
	import Picker, { type PickerItem } from '$lib/ui/Picker.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';
	import PageHeader from '$lib/ui/PageHeader.svelte';
	import DashboardRecap from './DashboardRecap.svelte';
	import DashboardToday from './DashboardToday.svelte';
	import { greeting } from './greeting';
	import QuickJournal from './QuickJournal.svelte';

	let { ws }: { ws: number } = $props();

	const { actors, agents } = pageStores();

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

<div class="flex flex-col px-(--page-inset) pb-10">
	<!-- The greeting is the page's title, in the header every page has; Quick journal, a note for
	     today's entry rather than a message, is a tool of the page beside it. -->
	<PageHeader title={hello}>
		{#snippet actions()}
			<Button variant="secondary" onclick={() => (journaling = true)}>
				<NotebookPen size={14} strokeWidth={1.75} aria-hidden="true" />Quick journal
			</Button>
		{/snippet}
	</PageHeader>

	<!-- One centred column, as a document's and a conversation's are: the composer, then what
	     needs the user today and what the agents and actors left. -->
	<div class="mx-auto flex w-full max-w-190 flex-col">
		<section aria-label="Message an agent" class="flex flex-col gap-5">
			{#if agents.list.loaded && list.length === 0}
				<p class="text-sm text-fg-subtle">
					No agents yet. <a href={routes.newAgent()}>New agent</a>
				</p>
			{/if}
			{#if agent}
				<Composer
					bind:value={draft}
					bind:images
					error={sendError}
					placeholder="Give it something to do…"
					onsend={send}
					onclearerror={() => (sendError = null)}
					{focusKey}
				>
					{#snippet leading()}
						<!-- Who the message goes to, inside the composer: a chip opening a picker of every
					     agent, so it holds however many there are. -->
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
									class="inline-flex h-9 cursor-pointer items-center gap-1.5 rounded-full pr-2 pl-1.5 text-sm text-fg transition-colors hover:bg-bg-hover data-[state=open]:bg-bg-hover"
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
									<ChevronDown
										size={14}
										strokeWidth={2}
										class="text-fg-subtle"
										aria-hidden="true"
									/>
								</button>
							{/snippet}
						</Picker>
					{/snippet}
				</Composer>
			{/if}
		</section>

		<QuickJournal {ws} bind:open={journaling} />

		<div class="flex flex-col gap-8 pt-8">
			<DashboardToday {ws} />
			<DashboardRecap />
		</div>
	</div>
</div>
