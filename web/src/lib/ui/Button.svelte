<!--
	A button in one of the app's few kinds: `primary`, filled in the accent,
	settles a dialog, a form or a card; `secondary`, outlined, is a page's own
	action in its header (New document, Resolve) and the header's menus (the
	order, Filters) wear the same outline through `secondaryClass`; `quiet`
	is everything else, as text; `danger-quiet` removes. A toolbar's icon
	buttons are drawn by their pages, square and quiet.
-->
<script lang="ts" module>
	/** The outline a header's own action wears, and the menus beside it that aren't Buttons. */
	export const secondaryClass =
		'border-border text-fg enabled:hover:bg-bg-hover data-[state=open]:bg-bg-hover';
</script>

<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { HTMLButtonAttributes } from 'svelte/elements';

	interface Props extends HTMLButtonAttributes {
		variant?: 'primary' | 'secondary' | 'quiet' | 'danger-quiet';
		children: Snippet;
	}

	let { variant = 'quiet', type = 'button', class: className, children, ...rest }: Props = $props();

	const variants = {
		quiet: 'border-transparent text-fg enabled:hover:bg-bg-hover',
		// Disabled, a main action keeps its fill, faded, so the page still has two kinds of button, not three.
		primary: 'border-transparent bg-primary text-primary-fg enabled:hover:bg-primary-hover',
		secondary: secondaryClass,
		'danger-quiet': 'border-transparent text-danger enabled:hover:bg-danger/10'
	};
</script>

<button
	class={[
		'inline-flex h-7.5 cursor-pointer items-center gap-1.5 rounded-sm border px-3 font-medium transition-colors disabled:cursor-default',
		'disabled:opacity-45',
		variants[variant],
		className
	]}
	{type}
	{...rest}>{@render children()}</button
>
