<!-- The tags on an item, as quiet pills in its row's meta line, each named as written. -->
<script lang="ts">
	import type { Tag } from '$lib/protocol/types';

	let { tags }: { tags: Tag[] } = $props();
</script>

{#each tags as tag (tag.id)}
	<span
		class="inline-flex h-4.5 flex-none items-center rounded-full bg-current-item px-1.75 text-2xs leading-none font-medium text-fg-muted"
		>{tag.name}</span
	>
{/each}
