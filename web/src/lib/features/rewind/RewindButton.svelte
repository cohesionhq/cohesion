<!--
	Rewind's symbol, the same on every item that has a history, and the panel it
	opens. `onopen` runs first, so an editor can save what is typed: a version
	can only hold what was saved.
-->
<script lang="ts">
	import History from '@lucide/svelte/icons/history';
	import type { Restored, RewindItem } from '$lib/stores/rewind.svelte';
	import RewindPanel from './RewindPanel.svelte';

	interface Props {
		item: RewindItem;
		/** The panel's title: the item's name. */
		title: string;
		onopen?: () => void;
		onrestored?: (restored: Restored) => void;
	}

	let { item, title, onopen, onrestored }: Props = $props();

	let open = $state(false);
</script>

<button
	class="inline-flex size-7.5 flex-none cursor-pointer items-center justify-center rounded-sm text-fg-muted transition-colors hover:bg-bg-hover hover:text-fg"
	type="button"
	aria-label="Rewind"
	onclick={() => {
		onopen?.();
		open = true;
	}}
>
	<History size={16} strokeWidth={1.75} aria-hidden="true" />
</button>
<RewindPanel bind:open {item} {title} {onrestored} />
