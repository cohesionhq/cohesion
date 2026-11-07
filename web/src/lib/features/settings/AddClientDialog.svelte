<script lang="ts">
	import { toProtocolError } from '$lib/protocol/errors';
	import { clients } from '$lib/stores/settings/clients.svelte';
	import Button from '$lib/ui/Button.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import { copyToClipboard } from '$lib/ui/toasts.svelte';

	interface Props {
		open?: boolean;
	}

	let { open = $bindable(false) }: Props = $props();

	let code = $state<string | null>(null);
	let error = $state<string | null>(null);

	// Each open asks for a new code, since the last one works once.
	$effect(() => {
		if (!open) return;
		code = null;
		error = null;
		let current = true;
		clients.beginPairing().then(
			(pairing) => {
				if (current) code = pairing.code;
			},
			(failure) => {
				if (current) error = toProtocolError(failure).message;
			}
		);
		return () => (current = false);
	});
</script>

<Dialog
	bind:open
	title="Add client"
	description="This code works once and expires in 10 minutes."
	onclosed={() => (code = null)}
>
	{#if error}
		<p class="text-danger">{error}</p>
	{:else if code}
		<p class="rounded-md border border-border bg-bg-raised p-3 font-mono text-xs break-all">
			{code}
		</p>
	{:else}
		<p class="text-fg-muted">Getting a code…</p>
	{/if}
	{#snippet footer()}
		<Button onclick={() => (open = false)}>Close</Button>
		<Button
			variant="primary"
			disabled={code === null}
			onclick={() => code && copyToClipboard(code, 'Copied the setup code.')}
		>
			Copy code
		</Button>
	{/snippet}
</Dialog>
