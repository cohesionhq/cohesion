<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { HTMLButtonAttributes } from 'svelte/elements';

	interface Props extends HTMLButtonAttributes {
		variant?: 'primary' | 'quiet' | 'danger-quiet';
		children: Snippet;
	}

	let { variant = 'quiet', type = 'button', class: className, children, ...rest }: Props = $props();

	const variants = {
		quiet: 'text-fg enabled:hover:bg-bg-hover',
		primary: 'bg-primary text-primary-fg enabled:hover:bg-primary-hover',
		'danger-quiet': 'text-danger enabled:hover:bg-danger/10'
	};
</script>

<button
	class={[
		'inline-flex h-7.5 cursor-pointer items-center gap-1 rounded-sm border border-transparent px-3 font-medium transition-colors disabled:cursor-default disabled:opacity-50',
		variants[variant],
		className
	]}
	{type}
	{...rest}>{@render children()}</button
>
