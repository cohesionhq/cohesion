<!--
	Imports records of one kind into the current workspace from its
	connector's accounts.
-->
<script lang="ts">
	import { connectors } from '$lib/stores/connectors.svelte';
	import Button from '$lib/ui/Button.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import ImportView from './ImportView.svelte';

	interface Props {
		open: boolean;
		kindId: number;
	}

	let { open = $bindable(), kindId }: Props = $props();

	const connector = $derived(connectors.kind(kindId)?.connector);
</script>

<Dialog bind:open title={connector ? `Import from ${connector.title}` : 'Import'} size="large">
	<ImportView {kindId} />
	{#snippet footer()}
		<Button class="border-border-strong!" onclick={() => (open = false)}>Done</Button>
	{/snippet}
</Dialog>
