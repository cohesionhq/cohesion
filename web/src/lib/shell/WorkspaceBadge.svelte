<!--
	A workspace's emoji, or the first letter of its title when it has none, in
	a small square. Decorative: the title always appears alongside it.
-->
<script lang="ts">
	import type { Workspace } from '$lib/protocol/types';

	let { workspace }: { workspace: Workspace | null } = $props();
</script>

{#if workspace?.emoji}
	<span
		class="inline-grid size-5 flex-none place-items-center text-sm leading-none"
		aria-hidden="true">{workspace.emoji}</span
	>
{:else}
	<span
		class="inline-grid size-5 flex-none place-items-center rounded-sm border border-border-strong text-2xs text-fg-muted uppercase"
		aria-hidden="true">{(workspace?.title ?? '·').slice(0, 1)}</span
	>
{/if}
