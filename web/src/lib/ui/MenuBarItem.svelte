<!--
	One chip of a MenuBar: flat text over an underline, naming what is
	picked, that opens the bar's panel onto `children`. The underline
	takes the accent while its panel is open.
-->
<script lang="ts">
	import { onDestroy, type Snippet } from 'svelte';
	import { getMenuBar, type MenuBarEntry } from './menuBar.svelte';

	interface Props {
		/** What is picked, as the chip shows it. */
		label: string;
		/** What the chip picks, for its accessible label ("Model"). */
		name: string;
		/** The panel's contents, handed a function that closes it. */
		children: Snippet<[close: () => void]>;
	}

	let { label, name, children }: Props = $props();

	const bar = getMenuBar();
	const id = $props.id();

	let button = $state<HTMLButtonElement>();
	const entry = $derived<MenuBarEntry | null>(
		button ? { id, anchor: button, panel: children } : null
	);
	const open = $derived(bar.open && bar.active?.id === id);

	onDestroy(() => bar.forget(id));
</script>

<button
	class="group inline-flex h-8 max-w-full min-w-0 cursor-pointer items-center text-sm text-fg-muted transition-colors hover:text-fg data-[state=open]:text-fg"
	type="button"
	bind:this={button}
	aria-haspopup="dialog"
	aria-expanded={open}
	aria-label="{name}: {label}"
	data-state={open ? 'open' : 'closed'}
	onclick={() => entry && bar.toggle(entry)}
	onpointerenter={() => entry && bar.hover(entry)}
>
	<!-- A border, not text-decoration: truncating clips an underline set below the text. -->
	<span
		class={[
			'min-w-0 truncate border-b pb-0.5 transition-colors',
			open ? 'border-accent' : 'border-border-strong group-hover:border-fg-subtle'
		]}>{label}</span
	>
</button>
