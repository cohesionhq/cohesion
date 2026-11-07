<!--
	A filter as a pill: a link to the list with that filter changed. A chosen
	pill takes the accent and a mark, which says which way it is chosen: a check
	while it keeps what matches it, an X and a strike-through while it hides that.
-->
<script lang="ts" module>
	/** What a filter pill does to the list: keeps only what matches it, hides what does, or nothing. */
	export type PillState = 'with' | 'without' | null;

	/** A filter pill's classes in each state, for a control drawn in the same shape. */
	export function pillClass(state: PillState) {
		return [
			'inline-flex h-6 items-center gap-1 rounded-full px-2.5 text-xs font-medium transition-colors hover:no-underline',
			state === null
				? 'bg-current-item text-fg-muted hover:bg-bg-hover hover:text-fg'
				: 'bg-accent-soft text-accent shadow-[inset_0_0_0_1px_color-mix(in_oklch,var(--accent)_40%,transparent)]',
			state === 'without' && 'line-through'
		];
	}
</script>

<script lang="ts">
	import Check from '@lucide/svelte/icons/check';
	import X from '@lucide/svelte/icons/x';
	import type { Snippet } from 'svelte';
	import type { ClassValue, MouseEventHandler } from 'svelte/elements';

	interface Props {
		label: string;
		filter: PillState;
		href: string;
		/** Drawn between the mark and the label. */
		icon?: Snippet;
		class?: ClassValue;
		onclick?: MouseEventHandler<HTMLAnchorElement>;
	}

	let { label, filter, href, icon, class: className, onclick }: Props = $props();
</script>

<a
	class={[pillClass(filter), 'flex-none', className]}
	{href}
	{onclick}
	aria-current={filter === 'with' ? 'true' : undefined}
>
	{#if filter === 'with'}
		<Check size={12} strokeWidth={2} aria-hidden="true" />
	{:else if filter === 'without'}
		<X size={12} strokeWidth={2} aria-hidden="true" />
	{/if}
	{@render icon?.()}
	{label}{#if filter === 'without'}<span class="sr-only">, hidden</span>{/if}
</a>
