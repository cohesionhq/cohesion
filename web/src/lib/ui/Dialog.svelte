<script lang="ts">
	import { Dialog } from 'bits-ui';
	import type { Snippet } from 'svelte';

	interface Props {
		open?: boolean;
		title: string;
		description?: string;
		children?: Snippet;
		/** Actions row, right-aligned. */
		footer?: Snippet;
		/** `wide` gives forms with a long text field more room; `large` fits a sidebar beside a list, which the children size and scroll themselves. */
		size?: 'default' | 'wide' | 'large';
		/** Centered on the dialog's top edge, half above it: e.g. the icon of a consent prompt. */
		hero?: Snippet;
		/** Drawn before the title, on its line: e.g. the avatar of the agent a message goes to. */
		leading?: Snippet;
		/** Centers its title and description and stacks the footer's actions full-width, first on top, for a prompt; it scrolls with the window when it outgrows it. */
		centered?: boolean;
		/** Called once the dialog has finished closing, fade included: reset its contents here, so the fade doesn't show them change. */
		onclosed?: () => void;
	}

	let {
		open = $bindable(false),
		title,
		description,
		children,
		footer,
		size = 'default',
		hero,
		leading,
		centered = false,
		onclosed
	}: Props = $props();
</script>

{#snippet body()}
	{#if hero}
		<!-- Straddles the top edge, half above it. -->
		<div class="absolute top-0 left-1/2 flex -translate-x-1/2 -translate-y-1/2">
			{@render hero()}
		</div>
	{/if}
	<Dialog.Title
		class={[
			'font-semibold',
			centered ? 'text-center text-lg' : 'text-md',
			leading && 'flex items-center gap-2'
		]}
	>
		{#if leading}{@render leading()}{/if}{title}
	</Dialog.Title>
	{#if description}
		<Dialog.Description class={['mt-1 text-fg-muted', centered && 'text-center']}
			>{description}</Dialog.Description
		>
	{/if}
	{#if children}
		<div class="mt-4">{@render children()}</div>
	{/if}
	{#if footer}
		<div
			class={[
				'mt-5 flex gap-2',
				centered ? 'flex-col [&>button]:h-9 [&>button]:justify-center' : 'justify-end'
			]}
		>
			{@render footer()}
		</div>
	{/if}
{/snippet}

<Dialog.Root bind:open onOpenChangeComplete={(isOpen) => isOpen || onclosed?.()}>
	<Dialog.Portal>
		<Dialog.Overlay class="overlay-backdrop" />
		{@const width = {
			default: 'w-[min(440px,calc(100vw-2*var(--gutter)))]',
			wide: 'w-[min(640px,calc(100vw-2*var(--gutter)))]',
			large: 'w-[min(960px,calc(100vw-2*var(--gutter)))]'
		}[size]}
		{#if centered}
			<!-- Centered ones sit in the window and, if they outgrow it, scroll with it without a bar, hero and all.
			     The wrapper stays mounted while closed, so it lets clicks through to the page or backdrop. -->
			<div class="pointer-events-none fixed inset-0 z-50 [scrollbar-width:none] overflow-y-auto">
				<div
					class={[
						'flex min-h-full items-center justify-center px-(--gutter) py-(--dialog-inset)',
						hero
							? 'pt-[calc(var(--dialog-inset)+var(--titlebar-inset)+2rem)]'
							: 'pt-[calc(var(--dialog-inset)+var(--titlebar-inset))]'
					]}
				>
					<Dialog.Content
						class={['overlay-surface pointer-events-auto relative p-5', hero && 'pt-10', width]}
					>
						{@render body()}
					</Dialog.Content>
				</div>
			</div>
		{:else}
			<!-- Centered in the window below the titlebar inset, at least --dialog-inset from either edge; a
			     form taller than that scrolls inside. A hero would be clipped, so it doesn't. -->
			<Dialog.Content
				class={[
					'overlay-surface fixed top-[calc(50%+var(--titlebar-inset)/2)] left-1/2 z-50 -translate-1/2 p-5',
					hero
						? 'pt-10'
						: 'max-h-[calc(100dvh-2*var(--dialog-inset)-var(--titlebar-inset))] overflow-y-auto',
					width
				]}
			>
				{@render body()}
			</Dialog.Content>
		{/if}
	</Dialog.Portal>
</Dialog.Root>
