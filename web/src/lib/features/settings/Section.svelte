<!--
	One block of a settings page: a heading, a one-line muted description,
	then rows, drawn with the class lists in `$lib/ui/list`.
-->
<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		title: string;
		description?: string;
		/** Quiet controls beside the heading. */
		actions?: Snippet;
		children: Snippet;
	}

	let { title, description, actions, children }: Props = $props();
</script>

<!-- `section` marks the block so the next one can space itself from it. -->
<section class="section [.section+&]:mt-8">
	<!-- Its first line sits level with the nav's first row; a hairline under it starts the rows, as a
	     list section's heading does elsewhere. -->
	<header class="flex items-end justify-between gap-3 border-b border-border pt-1 pb-3">
		<div>
			<h2 class="text-md">{title}</h2>
			{#if description}<p class="mt-1 text-fg-muted">{description}</p>{/if}
		</div>
		{#if actions}<div class="flex flex-none gap-1">{@render actions()}</div>{/if}
	</header>
	{@render children()}
</section>
