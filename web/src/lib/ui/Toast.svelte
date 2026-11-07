<script lang="ts">
	import { flip } from 'svelte/animate';
	import { fly } from 'svelte/transition';
	import AgentAvatar from '$lib/ui/AgentAvatar.svelte';
	import { toasts } from './toasts.svelte';
	import { motion } from './motion';
</script>

<div
	class="fixed right-4 bottom-4 z-60 flex max-w-[min(360px,calc(100vw-2*var(--gutter)))] flex-col items-end gap-2"
	role="status"
	aria-live="polite"
>
	{#each toasts.items as toast (toast.id)}
		<!-- A link toast goes somewhere when clicked; either kind is dismissed by a click. -->
		<svelte:element
			this={toast.href ? 'a' : 'button'}
			class={[
				'cursor-pointer rounded-md border bg-bg px-3 py-2 text-left',
				toast.href && 'hover:bg-bg-hover hover:no-underline',
				toast.tone === 'error'
					? 'border-[color-mix(in_srgb,var(--danger)_40%,var(--border))] text-danger'
					: 'border-border text-fg'
			]}
			href={toast.href}
			type={toast.href ? undefined : 'button'}
			role={toast.href ? undefined : 'button'}
			tabindex="0"
			onclick={() => toasts.dismiss(toast.id)}
			in:fly={{ y: 8, duration: motion.duration }}
			out:fly={{ x: 16, duration: motion.duration }}
			animate:flip={{ duration: motion.duration }}
		>
			{#if toast.avatar}
				<span class="flex items-center gap-2.5">
					<AgentAvatar
						name={toast.avatar.name}
						color={toast.avatar.color}
						emoji={toast.avatar.emoji}
						size="sm"
						tight
					/>
					<span>{toast.message}</span>
				</span>
			{:else if toast.title}
				<span class="flex flex-col">
					<span class="font-semibold">{toast.title}</span>
					<span>{toast.message}</span>
				</span>
			{:else}
				{toast.message}
			{/if}
		</svelte:element>
	{/each}
</div>
