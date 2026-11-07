<!--
	Which of its actor's modes a session runs in, as a chip naming the mode
	that opens onto a menu of them, each with its description. A new session
	picks one before it starts; a session under way switches with it.
-->
<script lang="ts">
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import type { ActorMode } from '$lib/protocol/types';
	import Menu from '$lib/ui/Menu.svelte';

	interface Props {
		modes: ActorMode[];
		/** The mode's identifier. */
		mode: string;
		onpick: (identifier: string) => void;
		disabled?: boolean;
	}

	let { modes, mode, onpick, disabled = false }: Props = $props();

	const title = $derived(modes.find((m) => m.identifier === mode)?.title ?? '');
</script>

<Menu
	align="start"
	items={modes.map((m) => ({
		id: m.identifier,
		label: m.title,
		detail: m.description ?? undefined,
		checked: m.identifier === mode,
		onSelect: () => onpick(m.identifier)
	}))}
>
	{#snippet trigger(props)}
		<button
			class="inline-flex h-8 max-w-full min-w-0 cursor-pointer items-center gap-1.5 rounded-full border border-border px-3 text-sm text-fg-muted transition-colors hover:border-border-strong hover:text-fg disabled:cursor-default disabled:opacity-50 data-[state=open]:border-border-strong data-[state=open]:text-fg"
			type="button"
			{...props}
			{disabled}
			aria-label="Mode: {title}"
		>
			<span class="min-w-0 truncate">{title}</span>
			<ChevronDown class="flex-none" size={14} strokeWidth={1.75} aria-hidden="true" />
		</button>
	{/snippet}
</Menu>
