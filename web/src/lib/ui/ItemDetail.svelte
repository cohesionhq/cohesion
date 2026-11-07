<!--
	One row of an `ItemPanel`'s details: its icon and what it is, then its
	value. A value that can be changed is a control of its own, `editable`
	styled, which opens a menu or a picker in place.
-->
<script lang="ts" module>
	/** The look of a value that changes in place: the value itself, which takes a hover. */
	export const editable =
		'-mx-2 inline-flex min-h-7 max-w-full cursor-pointer items-center gap-1.5 rounded-sm px-2 text-left hover:bg-bg-hover data-[state=open]:bg-bg-hover';
</script>

<script lang="ts">
	import type { Component, Snippet } from 'svelte';

	let {
		label,
		icon: Icon,
		children
	}: { label: string; icon: Component; children: Snippet } = $props();
</script>

<dt class="flex min-h-7 items-center gap-2 text-fg-muted">
	<Icon class="flex-none" size={15} strokeWidth={1.75} aria-hidden="true" />
	{label}
</dt>
<dd class="flex min-h-7 min-w-0 flex-wrap items-center gap-1.5 break-words text-fg">
	{@render children()}
</dd>
