<!-- A stored model thread, read-only: what the model saw and did, and what it cost. -->
<script lang="ts">
	import { untrack } from 'svelte';
	import Transcript from '$lib/features/threads/Transcript.svelte';
	import { usd } from '$lib/format/money';
	import { tokenCount, usageTotals } from '$lib/format/usage';
	import { routes } from '$lib/shell/routes';
	import { transcript } from '$lib/stores/transcript';
	import PageHeader from '$lib/ui/PageHeader.svelte';
	import ResourceState from '$lib/ui/ResourceState.svelte';
	import { pane } from '$lib/shell/pane.svelte';
	import PageTitle from '$lib/shell/PageTitle.svelte';
	import { pageStores } from '$lib/stores/pageStores.svelte';

	const { threads } = pageStores();

	const page = pane();

	const id = $derived(Number(page.params.thread));

	$effect(() => {
		const thread = id;
		untrack(() => void threads.open(thread));
		return () => threads.current.release();
	});
</script>

<PageTitle title="Transcript" />

<div class="px-(--page-inset)">
	<ResourceState resource={threads.current} missing="Transcript not found">
		{#snippet loaded(detail)}
			{@const totals = usageTotals(detail.cost_events)}
			<PageHeader title="Transcript" back={{ href: routes.agents(), label: 'Agents' }}>
				{#snippet meta()}
					{#if detail.cost_events.length > 0}
						<span class="tabular-nums"
							>{usd(totals.cost)} · {tokenCount(totals.input_tokens)} in · {tokenCount(
								totals.cached_tokens
							)} cached · {tokenCount(totals.output_tokens)} out</span
						>
					{/if}
				{/snippet}
			</PageHeader>
			<Transcript entries={transcript(detail.messages)} />
		{/snippet}
	</ResourceState>
</div>
