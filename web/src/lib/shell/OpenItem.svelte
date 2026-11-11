<!-- What an Open page shows: finds a linked item and goes to it, in place of the page, or says it was not found. -->
<script lang="ts">
	import { untrack } from 'svelte';
	import { toProtocolError } from '$lib/protocol/errors';
	import { ITEM_KINDS, type ItemKind } from '$lib/shell/routes';
	import EmptyState from '$lib/ui/EmptyState.svelte';
	import { pane } from '$lib/shell/pane.svelte';

	const page = pane();

	interface Props {
		kind: ItemKind;
		/** Where the item opens, or null when it is not found. */
		find: () => Promise<string | null>;
	}

	let { kind, find }: Props = $props();

	let failure = $state<{ message: string; retry: boolean } | null>(null);

	$effect(() => {
		const [k, f] = [kind, find];
		untrack(() => void open(k, f));
	});

	async function open(k: ItemKind, f: () => Promise<string | null>) {
		failure = null;
		try {
			const href = await f();
			if (href === null) failure = { message: `${ITEM_KINDS[k].noun} not found.`, retry: false };
			else await page.goto(href, { replaceState: true });
		} catch (error) {
			failure = { message: toProtocolError(error).message, retry: true };
		}
	}
</script>

<div class="px-(--page-inset)">
	{#if failure}
		<EmptyState
			message={failure.message}
			actionLabel={failure.retry ? 'Try again' : undefined}
			onaction={failure.retry ? () => open(kind, find) : undefined}
		/>
	{/if}
</div>
