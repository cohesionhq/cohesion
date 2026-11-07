<!-- A draft's save state beside its page's actions, in words: "Saved" for a moment after each
     save, then nothing, and "Not saved" for as long as it couldn't be. -->
<script lang="ts">
	import { fade } from 'svelte/transition';
	import type { Draft } from '$lib/stores/draft.svelte';
	import { motion } from '$lib/ui/motion';

	let { draft }: { draft: Draft } = $props();

	/** How long "Saved" stays after a save. */
	const SHOWN_FOR = 2000;

	// Shown when a save finishes, not when a draft loads already saved.
	let recent = $state(false);
	let wasSaving = false;
	$effect(() => {
		if (draft.saving) {
			wasSaving = true;
			return;
		}
		if (!wasSaving) return;
		wasSaving = false;
		recent = true;
		const timer = setTimeout(() => (recent = false), SHOWN_FOR);
		return () => clearTimeout(timer);
	});

	const saved = $derived(recent && !draft.saveError && !draft.saving && !draft.dirty);
</script>

<span class="flex px-1 text-xs whitespace-nowrap text-fg-subtle" aria-live="polite">
	{#if draft.saveError}
		<span class="text-danger">Not saved</span>
	{:else if saved}
		<span transition:fade={{ duration: motion.duration }}>Saved</span>
	{/if}
</span>
