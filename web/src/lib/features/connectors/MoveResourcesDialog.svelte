<!--
	Moves every resource of one account to another account of its connector,
	picked here, in every workspace: how an orphaned account's resources
	reach a live one.
-->
<script lang="ts">
	import type { RedactedConnectorAccount } from '$lib/protocol/types';
	import { connectors } from '$lib/stores/connectors.svelte';
	import Button from '$lib/ui/Button.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import Field from '$lib/ui/Field.svelte';
	import Select from '$lib/ui/Select.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';

	interface Props {
		/** The account whose resources move; the dialog is open while it is set. */
		source: RedactedConnectorAccount | null;
		onclose: () => void;
	}

	let { source, onclose }: Props = $props();

	let target = $state('');
	let busy = $state(false);
	let open = $state(false);

	const targets = $derived(source ? connectors.siblings(source) : []);
	const options = $derived(targets.map((a) => ({ value: String(a.id), label: a.description })));

	$effect(() => {
		if (source) {
			open = true;
			target = String(targets[0]?.id ?? '');
		}
	});

	async function move() {
		const to = targets.find((a) => String(a.id) === target);
		if (!source || !to) return;
		busy = true;
		try {
			await connectors.migrate(source.id, to.id);
			toasts.show(`Moved ${source.description}’s resources to ${to.description}.`);
			open = false;
		} catch (error) {
			toasts.error(error);
		} finally {
			busy = false;
		}
	}
</script>

<Dialog
	bind:open
	onclosed={onclose}
	title="Move resources from {source?.description ?? 'account'}?"
	description="Resources will move to the account you pick, in every workspace. If both accounts have the same item, the copy in the account you pick is kept."
>
	<Field label="Account" for="move-resources-target">
		<Select id="move-resources-target" bind:value={target} {options} />
	</Field>
	{#snippet footer()}
		<Button class="border-border-strong!" onclick={() => (open = false)}>Cancel</Button>
		<Button variant="primary" onclick={move} disabled={busy || target === ''}>Move</Button>
	{/snippet}
</Dialog>
