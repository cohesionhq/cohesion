<!--
	A skill as a chip, drawn like a forwarded item's: a tile with the skills
	icon, then its name. In the composer it has a button taking it off; in the
	conversation it opens the prompt the message ran, as it stood when sent.
-->
<script lang="ts">
	import WandSparkles from '@lucide/svelte/icons/wand-sparkles';
	import X from '@lucide/svelte/icons/x';
	import Popover from '$lib/ui/Popover.svelte';

	interface Props {
		name: string;
		/** The prompt the chip opens onto in the conversation. */
		prompt: string;
		/** Takes the skill off the composer; without it, the chip opens its prompt. */
		ondismiss?: () => void;
	}

	let { name, prompt, ondismiss }: Props = $props();

	let open = $state(false);
	const chip =
		'inline-flex h-9 max-w-full min-w-0 items-center gap-2 rounded-md border border-border-strong bg-bg-hover pl-0.75 text-sm text-fg';
</script>

{#snippet body()}
	<span
		class="inline-flex size-7 flex-none items-center justify-center rounded-sm border border-border bg-bg text-fg-muted"
	>
		<WandSparkles size={15} strokeWidth={1.75} aria-hidden="true" />
	</span>
	<span class="min-w-0 truncate font-medium">{name}</span>
	<span class="flex-none text-xs text-fg-subtle">Skill</span>
{/snippet}

{#if ondismiss}
	<span class={[chip, 'pr-0.75']}>
		{@render body()}
		<button
			class="-ml-1 inline-flex size-7 flex-none cursor-pointer items-center justify-center rounded-sm text-fg-muted transition-colors hover:bg-n3 hover:text-fg"
			type="button"
			aria-label="Don’t use {name}"
			onclick={ondismiss}
		>
			<X size={14} strokeWidth={2} />
		</button>
	</span>
{:else}
	<Popover bind:open align="end">
		{#snippet trigger(props)}
			<button
				class={[chip, 'cursor-pointer pr-3 transition-colors hover:bg-n3 data-[state=open]:bg-n3']}
				type="button"
				{...props}
			>
				{@render body()}
			</button>
		{/snippet}
		<div class="flex w-96 max-w-full flex-col gap-1 px-2 py-1.5">
			<p class="text-xs text-fg-subtle">{name}</p>
			<p class="wrap-anywhere whitespace-pre-wrap">{prompt}</p>
		</div>
	</Popover>
{/if}
