<!--
	One permission or hook: what it allows or when it fires, with an icon
	and a quieter line under it when given. `control` sits at the trailing
	edge (a switch on an extension's page); `flag` marks the row (e.g. one a
	registry didn't list).
-->
<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { GrantIcon } from './grants';

	interface Props {
		icon?: GrantIcon;
		title: string;
		detail?: string;
		flag?: string;
		/** Dims the row, e.g. for a grant that is off. */
		muted?: boolean;
		control?: Snippet;
	}

	let { icon: Icon, title, detail, flag, muted = false, control }: Props = $props();
</script>

<li class="flex items-center gap-3 px-3 py-2.5">
	{#if Icon}
		<span
			class={[
				'flex size-7 flex-none items-center justify-center rounded-full bg-bg-hover transition-colors',
				muted ? 'text-fg-subtle' : 'text-fg'
			]}
		>
			<Icon size={15} strokeWidth={1.75} aria-hidden="true" />
		</span>
	{/if}
	<span class="flex min-w-0 flex-1 flex-col">
		<span class="flex items-center gap-2">
			<span class={['font-medium transition-colors', muted && 'text-fg-muted']}>{title}</span>
			{#if flag}
				<span
					class="inline-flex h-5 items-center rounded-sm bg-warn/12 px-1.5 text-2xs font-medium text-warn"
					>{flag}</span
				>
			{/if}
		</span>
		{#if detail}
			<span class="text-xs wrap-anywhere text-fg-subtle">{detail}</span>
		{/if}
	</span>
	{#if control}{@render control()}{/if}
</li>
