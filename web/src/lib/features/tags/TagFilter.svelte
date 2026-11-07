<!--
	A row of a kind's tags under a page's header. A tag either narrows the list
	to what carries it, hides what carries it, or does neither;
	the page decides which, and where picking a tag goes from there. A row too
	long for the width scrolls sideways. Hidden while there is nothing to show;
	"Manage tags…" ends the row.
-->
<script lang="ts" module>
	/** The classes of a row of filter pills: one line, scrolling sideways. */
	export const filterRowClass =
		'flex items-center gap-1.5 overflow-x-auto [scrollbar-width:none] [&::-webkit-scrollbar]:hidden';
</script>

<script lang="ts">
	import type { Tag } from '$lib/protocol/types';
	import FilterPill, { type PillState } from './FilterPill.svelte';

	interface Props {
		tags: Tag[];
		state: (tag_id: number) => PillState;
		/** Where picking a tag goes. */
		href: (tag_id: number) => string;
		onmanage: () => void;
	}

	let { tags, state, href, onmanage }: Props = $props();
</script>

{#if tags.length > 0}
	<div class={[filterRowClass, 'mb-4']}>
		<nav class="contents" aria-label="Tags">
			{#each tags as tag (tag.id)}
				<FilterPill label={tag.name} filter={state(tag.id)} href={href(tag.id)} />
			{/each}
		</nav>
		<button
			class="flex-none cursor-pointer px-1.5 py-0.5 text-xs whitespace-nowrap text-fg-subtle hover:text-fg"
			type="button"
			onclick={onmanage}>Manage tags…</button
		>
	</div>
{/if}
