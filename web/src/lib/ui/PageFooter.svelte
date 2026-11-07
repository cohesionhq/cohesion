<!-- A page's footer: links and actions that belong at the bottom of the page. The way back lives in PageHeader. -->
<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		/** The landmark's name. */
		label: string;
		children: Snippet;
	}

	let { label, children }: Props = $props();
</script>

<nav class="mt-6 flex items-center gap-2 border-t border-border pt-3 text-xs" aria-label={label}>
	{@render children()}
</nav>
