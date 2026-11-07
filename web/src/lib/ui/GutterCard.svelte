<!--
	The summary at the top of the column right of the sheet: a row flat on the
	canvas like the groups under it, as tall as a conversation's header so its
	hairline lines up with the header's, and inset as the groups are.
-->
<script lang="ts">
	import type { Snippet } from 'svelte';

	let { children }: { children: Snippet } = $props();
</script>

<div class="mx-2 flex min-h-14 flex-none flex-col justify-center gap-1 border-b border-border py-2">
	{@render children()}
</div>
