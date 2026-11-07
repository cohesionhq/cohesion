<!-- A draft's save state beside its page's actions: a check once it is saved, an alert when it couldn't be. -->
<script lang="ts">
	import Check from '@lucide/svelte/icons/check';
	import CircleAlert from '@lucide/svelte/icons/circle-alert';
	import type { Draft } from '$lib/stores/draft.svelte';

	let { draft }: { draft: Draft } = $props();

	const saved = $derived(
		!draft.saveError && !draft.saving && !draft.dirty && draft.savedAt !== null
	);
</script>

<span class="flex text-fg-subtle" aria-live="polite">
	{#if draft.saveError}
		<CircleAlert class="text-danger" size={14} strokeWidth={1.75} aria-label="Not saved" />
	{:else if saved}
		<Check size={14} strokeWidth={2} aria-label="Saved" />
	{/if}
</span>
