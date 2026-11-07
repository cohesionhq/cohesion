<script lang="ts">
	import Button from './Button.svelte';

	interface Props {
		/** The title: what's empty or missing. */
		message: string;
		/** The subtitle under it: what to do about it. */
		detail?: string;
		actionLabel?: string;
		onaction?: () => void;
		/** Use the filled button: only when this is the screen's one primary action. */
		primary?: boolean;
		/** Less room above and below, for a list whose own controls sit just above. */
		compact?: boolean;
		/** Fills the rest of a page that is a full-height flex column, and sits in its middle. */
		centered?: boolean;
	}

	let {
		message,
		detail,
		actionLabel,
		onaction,
		primary = false,
		compact = false,
		centered = false
	}: Props = $props();
</script>

<div
	class={[
		'flex flex-col items-center gap-3 text-center text-fg-muted',
		centered && 'flex-1 justify-center',
		compact ? 'py-4' : 'py-12'
	]}
>
	<div class="flex flex-col gap-1">
		<p>{message}</p>
		{#if detail}<p class="text-sm text-fg-subtle">{detail}</p>{/if}
	</div>
	{#if actionLabel && onaction}
		<Button variant={primary ? 'primary' : 'quiet'} onclick={onaction}>{actionLabel}</Button>
	{/if}
</div>
