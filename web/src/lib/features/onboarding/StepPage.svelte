<!--
	One page of the first-run flow: a small line above a big title, a line or
	two under it, the page's own content, and a row of actions. The title is
	set larger than anywhere else in the app, so the flow reads as its own
	moment rather than another dialog. The card is tall and narrow, so the
	page stacks: its art over a centred title, then its content, then its
	actions at the foot.
-->
<script lang="ts">
	import { Dialog } from 'bits-ui';
	import type { Snippet } from 'svelte';
	import Button from '$lib/ui/Button.svelte';

	interface Props {
		eyebrow: string;
		title: string;
		description: string;
		/** Drawn beside the title, such as a preview of what the page makes. */
		art?: Snippet;
		children?: Snippet;
		/** The page's main action, at the end of the row. */
		action?: Snippet;
		/** Steps back a page; absent on the first. */
		onback?: () => void;
		/** Moves on without doing the page's step; absent when there's nothing to skip. */
		onskip?: () => void;
	}

	let { eyebrow, title, description, art, children, action, onback, onskip }: Props = $props();
</script>

<div class="flex flex-1 flex-col gap-5">
	<div class="flex flex-col items-center gap-2 text-center">
		{#if art}<div class="mb-2 flex-none">{@render art()}</div>{/if}
		<p class="text-xs font-medium text-accent">{eyebrow}</p>
		<Dialog.Title
			class="text-display leading-[1.05] font-bold tracking-[-0.03em] text-balance max-sm:text-xl"
		>
			{title}
		</Dialog.Title>
		<Dialog.Description class="text-md text-pretty text-fg-muted">{description}</Dialog.Description>
	</div>

	{#if children}{@render children()}{/if}

	<!-- Pinned to the foot of the tall card, so each page's actions sit in the same place. -->
	<div class="mt-auto flex flex-col gap-2">
		{#if action}
			<div class="flex flex-col [&>button]:h-9 [&>button]:justify-center">{@render action()}</div>
		{/if}
		{#if onback || onskip}
			<div class="flex items-center">
				{#if onback}<Button onclick={onback}>Back</Button>{/if}
				<div class="flex-1"></div>
				{#if onskip}<Button onclick={onskip}>Skip for now</Button>{/if}
			</div>
		{/if}
	</div>
</div>
