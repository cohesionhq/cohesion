<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		label: string;
		/** Id of the control, so the label points at it. */
		for?: string;
		help?: string;
		error?: string | null;
		children: Snippet;
	}

	let { label, for: htmlFor, help, error = null, children }: Props = $props();
</script>

<div class="flex flex-col gap-1">
	<label class="font-medium" for={htmlFor}>{label}</label>
	{@render children()}
	{#if error}
		<p class="text-xs text-danger">{error}</p>
	{:else if help}
		<p class="text-xs text-fg-subtle">{help}</p>
	{/if}
</div>
