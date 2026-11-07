<!--
	The summary at the top of the column right of the sheet: a small sheet of
	its own on the canvas, with the sheet's corners and at least the height of
	a conversation's header, so it lines up with both.
-->
<script lang="ts">
	import type { Snippet } from 'svelte';

	let { children }: { children: Snippet } = $props();
</script>

<div
	class="flex min-h-14 flex-none flex-col justify-center gap-1 rounded-(--sheet-radius) bg-bg px-4 py-3 shadow-sheet"
>
	{@render children()}
</div>
