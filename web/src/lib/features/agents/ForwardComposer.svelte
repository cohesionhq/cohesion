<!--
	The composer "Forward" opens: a panel floating over the foot of the page
	the item is on, not a dialog, so the page stays where it is and usable.
	It shows the agent, the item (which can be taken off) and an optional
	note; the agent gets the note and the item's link, which it reads with
	its code tool. Escape, the close button or a click
	outside closes it. A toast says it went; a failure shows here, with the
	note and item kept.
-->
<script lang="ts">
	import X from '@lucide/svelte/icons/x';
	import { fly } from 'svelte/transition';
	import { toProtocolError } from '$lib/protocol/errors';
	import AgentAvatar from '$lib/ui/AgentAvatar.svelte';
	import Button from '$lib/ui/Button.svelte';
	import Textarea from '$lib/ui/Textarea.svelte';
	import { motion } from '$lib/ui/motion';
	import { toasts } from '$lib/ui/toasts.svelte';
	import { forwardTarget, forwardedTitle, forwarding } from './forward.svelte';
	import ForwardChip from './ForwardChip.svelte';
	import { pageStores } from '$lib/stores/pageStores.svelte';

	const { agents } = pageStores();

	const id = $props.id();
	let note = $state('');
	let error = $state<string | null>(null);
	let busy = $state(false);
	let panel = $state<HTMLElement>();
	let ref = $state<HTMLTextAreaElement>();

	const draft = $derived(forwarding.draft);
	const canSend = $derived(!busy && (draft?.item != null || note.trim() !== ''));

	// Each agent chosen in a "Forward" menu starts a fresh note, focused once
	// the menu has closed and handed focus back to its trigger.
	$effect(() => {
		if (!forwarding.draft) return;
		note = '';
		error = null;
		const frame = requestAnimationFrame(() => ref?.focus());
		return () => cancelAnimationFrame(frame);
	});

	function close() {
		if (!busy) forwarding.draft = null;
	}

	async function send() {
		if (!draft || !canSend) return;
		const { agent, item } = draft;
		busy = true;
		error = null;
		try {
			await agents.sendMessage(
				agent.id,
				note.trim(),
				[],
				item ? { item, target: forwardTarget(item) } : undefined
			);
			forwarding.draft = null;
			toasts.show(
				item
					? `Forwarded “${forwardedTitle(item)}” to ${agent.agent_name}.`
					: `Sent to ${agent.agent_name}.`,
				'info',
				{ avatar: { name: agent.agent_name, color: agent.agent_color, emoji: agent.agent_emoji } }
			);
		} catch (e) {
			error = toProtocolError(e).message;
		} finally {
			busy = false;
		}
	}

	// As the agent's own composer: Enter sends, Shift+Enter starts a line.
	function onkeydown(event: KeyboardEvent) {
		if (event.key !== 'Enter' || event.isComposing) return;
		if (event.shiftKey && !(event.metaKey || event.ctrlKey)) return;
		event.preventDefault();
		void send();
	}

	function onpanelkeydown(event: KeyboardEvent) {
		if (event.key !== 'Escape') return;
		event.preventDefault();
		close();
	}

	function onwindowpointerdown(event: PointerEvent) {
		if (draft && panel && !panel.contains(event.target as Node)) close();
	}
</script>

<svelte:window onpointerdown={onwindowpointerdown} />

{#if draft}
	{@const { agent } = draft}
	<div
		role="dialog"
		tabindex="-1"
		class="overlay-surface fixed bottom-(--gutter) left-1/2 z-45 flex w-[min(32rem,calc(100vw-2*var(--gutter)))] -translate-x-1/2 flex-col gap-2 p-3"
		aria-labelledby="{id}-title"
		bind:this={panel}
		data-claims-focus
		onkeydown={onpanelkeydown}
		in:fly={{ y: 8, duration: motion.duration }}
	>
		<header class="flex items-center gap-2">
			<AgentAvatar
				name={agent.agent_name}
				color={agent.agent_color}
				emoji={agent.agent_emoji}
				size="xs"
			/>
			<h2 class="min-w-0 flex-1 truncate font-medium" id="{id}-title">
				Forward to {agent.agent_name}
			</h2>
			<button
				class="inline-flex size-6 flex-none cursor-pointer items-center justify-center rounded-sm text-fg-muted transition-colors hover:bg-bg-hover hover:text-fg"
				type="button"
				aria-label="Close"
				onclick={close}
			>
				<X size={14} strokeWidth={2} />
			</button>
		</header>
		{#if draft.item}
			<div class="flex">
				<ForwardChip item={draft.item} ondismiss={() => (draft.item = null)} />
			</div>
		{/if}
		<Textarea
			bind:value={note}
			bind:ref
			rows={3}
			class="max-h-[40vh] resize-none"
			aria-label="Note"
			aria-describedby={error ? `${id}-error` : undefined}
			placeholder={draft.item ? 'Add a note…' : `Message ${agent.agent_name}…`}
			{onkeydown}
		/>
		{#if error}
			<p class="text-xs text-danger" id="{id}-error" role="alert">{error}</p>
		{/if}
		<div class="flex justify-end">
			<Button variant="primary" disabled={!canSend} onclick={send}>
				{#if draft.item}{busy ? 'Forwarding…' : 'Forward'}{:else}{busy ? 'Sending…' : 'Send'}{/if}
			</Button>
		</div>
	</div>
{/if}
