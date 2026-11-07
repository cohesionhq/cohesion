<!--
	One group in the column right of the sheet, flat on the canvas, set apart
	from the group above by a hairline, and inset a little from the card's edges.
-->
<script lang="ts">
	import type { Snippet } from 'svelte';

	let { children }: { children: Snippet } = $props();
</script>

<div
	class="mx-2 flex flex-none flex-col gap-1.5 pt-4 [&+&]:mt-3 [&+&]:border-t [&+&]:border-border [&+&]:pt-3"
>
	{@render children()}
</div>
