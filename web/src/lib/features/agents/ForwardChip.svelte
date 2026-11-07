<!--
	A forwarded item as a chip set apart from the messages around it: its
	kind's icon (the sidebar's) on a tile, the title, then `Task #42` quietly.
	In the composer it has a button taking it off; in the conversation it is
	the item's `cohesion://` link, opening as one does in Markdown: in its
	panel over the page.
-->
<script lang="ts">
	import CheckSquare from '@lucide/svelte/icons/square-check';
	import FileText from '@lucide/svelte/icons/file-text';
	import NotebookPen from '@lucide/svelte/icons/notebook-pen';
	import X from '@lucide/svelte/icons/x';
	import { linkedItem } from '$lib/shell/linkedItem.svelte';
	import { itemLinkRoute } from '$lib/shell/routes';
	import {
		type ForwardedItem,
		forwardedKind,
		forwardedLabel,
		forwardedLink,
		forwardedTitle
	} from './forward.svelte';

	interface Props {
		item: ForwardedItem;
		/** Takes the item off the composer; without it, the chip links to the item. */
		ondismiss?: () => void;
	}

	let { item, ondismiss }: Props = $props();

	const icons = { task: CheckSquare, document: FileText, journal: NotebookPen };
	const Icon = $derived(icons[item.kind]);
	const label = $derived(forwardedLabel(item));
	const link = $derived(forwardedLink(item));
	// The tile and the dismiss button sit 4px inside the chip's 10px corners
	// (3px and the border), so they take 6px.
	const chip =
		'inline-flex h-9 max-w-full min-w-0 items-center gap-2 rounded-md border border-border-strong bg-bg-hover pl-0.75 text-sm text-fg';
</script>

{#snippet body()}
	<span
		class="inline-flex size-7 flex-none items-center justify-center rounded-sm border border-border bg-bg text-fg-muted"
	>
		<Icon size={15} strokeWidth={1.75} aria-hidden="true" />
	</span>
	<span class="min-w-0 truncate font-medium">{forwardedTitle(item)}</span>
	<span class="flex-none text-xs text-fg-subtle">{forwardedKind(item)}</span>
{/snippet}

{#if ondismiss}
	<span class={[chip, 'pr-0.75']}>
		{@render body()}
		<button
			class="-ml-1 inline-flex size-7 flex-none cursor-pointer items-center justify-center rounded-sm text-fg-muted transition-colors hover:bg-n3 hover:text-fg"
			type="button"
			aria-label="Don’t forward {label}"
			onclick={ondismiss}
		>
			<X size={14} strokeWidth={2} />
		</button>
	</span>
{:else}
	<a
		class={[chip, 'pr-3 no-underline transition-colors hover:bg-n3']}
		href={itemLinkRoute(link)}
		onclick={(event) => linkedItem.follow(event, link)}
	>
		{@render body()}
	</a>
{/if}
