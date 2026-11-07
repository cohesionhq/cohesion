<!--
	Something an actor asks leave to do, inline in the transcript: the command
	and where it runs, the files and how each changes, or the tool and its
	input; why, in its words; and one button per decision it takes, with a
	note to send along. Once decided, the card is read-only and the decision
	follows it.
-->
<script lang="ts">
	import type { ActorPermissionDecision, ActorPermissionRequest } from '$lib/protocol/types';
	import Button from '$lib/ui/Button.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';
	import { decisionLabel, fileChangeLabel } from './words';

	interface Props {
		request: ActorPermissionRequest;
		answered: boolean;
		ondecide: (decision: ActorPermissionDecision, message: string | null) => Promise<unknown>;
	}

	let { request, answered, ondecide }: Props = $props();

	let note = $state('');
	let busy = $state(false);

	const title = $derived.by(() => {
		switch (request.subject.kind) {
			case 'command':
				return 'Run a command?';
			case 'fileChange':
				return request.subject.changes.length === 1
					? 'Change a file?'
					: `Change ${request.subject.changes.length} files?`;
			case 'tool':
				return `Use ${request.subject.name}?`;
		}
	});

	async function decide(decision: ActorPermissionDecision) {
		busy = true;
		try {
			await ondecide(decision, note.trim() || null);
		} catch (error) {
			toasts.error(error);
		} finally {
			busy = false;
		}
	}

	const pre =
		'max-h-80 overflow-auto rounded-sm bg-bg-raised px-2 py-1.5 font-mono text-xs whitespace-pre-wrap';
</script>

<article
	class="flex w-full max-w-[70%] min-w-0 flex-col gap-2 rounded-md border border-border bg-bg px-3 py-2.5 max-sm:max-w-full"
>
	<p class="font-medium">{title}</p>
	{#if request.subject.kind === 'command'}
		<pre class={pre}>{request.subject.command}</pre>
		{#if request.subject.cwd}
			<p class="font-mono text-xs wrap-anywhere text-fg-subtle">in {request.subject.cwd}</p>
		{/if}
	{:else if request.subject.kind === 'fileChange'}
		<ul class="flex flex-col gap-2">
			{#each request.subject.changes as change (change.path)}
				<li class="flex flex-col gap-1">
					<span class="text-xs"
						><span class="text-fg-subtle">{fileChangeLabel(change.change)}</span>
						<span class="font-mono wrap-anywhere">{change.path}</span></span
					>
					{#if change.diff}<pre class={pre}>{change.diff}</pre>{/if}
				</li>
			{/each}
		</ul>
	{:else}
		<pre class={pre}>{JSON.stringify(request.subject.input, null, 2)}</pre>
	{/if}
	{#if request.reason}
		<p class="text-sm wrap-anywhere text-fg-muted">{request.reason}</p>
	{/if}
	{#if !answered}
		<TextInput bind:value={note} placeholder="Add a note (optional)" aria-label="Note" />
		<div class="flex flex-wrap justify-end gap-2">
			{#each request.decisions as decision (decision)}
				<Button
					variant={decision === 'allow' ? 'primary' : undefined}
					disabled={busy}
					onclick={() => decide(decision)}>{decisionLabel(decision)}</Button
				>
			{/each}
		</div>
	{/if}
</article>
