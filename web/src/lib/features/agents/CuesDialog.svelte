<!--
	The cues an agent set to wake itself: for each, when it fires, what it
	does then, which tier's model its runs use and the check it runs.
	Remove takes one away after a confirmation.
-->
<script lang="ts">
	import type { IntelligenceAgentCue } from '$lib/protocol/types';
	import { cueTitle, hookLabel } from '$lib/stores/agents.svelte';
	import Button from '$lib/ui/Button.svelte';
	import ConfirmDialog from '$lib/ui/ConfirmDialog.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import { preClass, summaryClass } from './disclosure';

	interface Props {
		open?: boolean;
		cues: IntelligenceAgentCue[];
		/** Removes cue `cue_id`; the confirmation closes when it resolves. */
		onremove: (cue_id: number) => Promise<unknown>;
	}

	let { open = $bindable(false), cues, onremove }: Props = $props();

	let removing = $state<IntelligenceAgentCue | null>(null);
</script>

<Dialog bind:open title="Cues" description="The agent set these to wake itself up." size="wide">
	<div class="flex max-h-[60vh] flex-col overflow-y-auto">
		{#each cues as cue (cue.id)}
			{@const title = cueTitle(cue.name, cue.hook)}
			{@const when = `When ${hookLabel(cue.hook)}`}
			<section class="flex flex-col gap-2 border-b border-border py-4 first:pt-0 last:border-b-0">
				<div class="flex items-start gap-3">
					<div class="flex min-w-0 flex-1 flex-col gap-0.5">
						<h3 class="text-sm font-medium wrap-anywhere">{title}</h3>
						<p class="text-xs text-fg-subtle">
							{#if title !== when}{when} ·
							{/if}<span class="capitalize">{cue.effort}</span> effort
						</p>
					</div>
					<Button variant="danger-quiet" onclick={() => (removing = cue)}>Remove…</Button>
				</div>
				<p class="wrap-anywhere whitespace-pre-wrap">{cue.instructions}</p>
				<details class="group">
					<summary class={summaryClass}>Check</summary>
					<p class="mb-1 text-xs text-fg-subtle">
						This check runs each time the cue fires. The agent wakes only when the check returns
						something, and the agent receives that result.
					</p>
					<pre class={[preClass, 'text-fg-muted']}>{cue.script}</pre>
				</details>
			</section>
		{:else}
			<p class="text-fg-subtle">No cues yet.</p>
		{/each}
	</div>
	{#snippet footer()}
		<Button onclick={() => (open = false)}>Close</Button>
	{/snippet}
</Dialog>

<ConfirmDialog
	subject={removing}
	onclose={() => (removing = null)}
	title={removing ? `Remove “${cueTitle(removing.name, removing.hook)}”?` : 'Remove the cue?'}
	description="The agent will stop waking for this cue. The agent’s other cues won’t be removed."
	confirmLabel="Remove"
	onconfirm={(cue) => onremove(cue.id)}
/>
