<!--
	Settings shell: a section nav (a column from 900px, scrolling tabs
	below) beside the current section, drawn as the Tasks and Articles
	pages draw their folders: the same width, no icons, the same current tint.
-->
<script lang="ts">
	import type { Snippet } from 'svelte';
	import { SECTIONS } from '$lib/features/settings/sections';
	import { routes } from '$lib/shell/routes';
	import PageHeader from '$lib/ui/PageHeader.svelte';
	import { pane } from '$lib/shell/pane.svelte';

	const page = pane();

	let { children }: { children: Snippet } = $props();

	const current = $derived(page.params.section ?? 'workspaces');
</script>

<div class="px-(--page-inset)">
	<PageHeader title="Settings" />
	<div
		class="grid grid-cols-[var(--sidebar-width)_minmax(0,1fr)] items-start gap-6 max-md:grid-cols-[minmax(0,1fr)] max-md:gap-5"
	>
		<!-- Pinned under the page header, where it starts, so it never moves. -->
		<nav
			class="sticky top-(--page-header-height) flex flex-col gap-px max-md:static max-md:-mx-(--page-inset) max-md:-mt-3 max-md:[scrollbar-width:none] max-md:flex-row max-md:gap-1 max-md:overflow-x-auto max-md:overscroll-y-auto max-md:border-b max-md:border-border max-md:px-(--page-inset) max-md:pb-2"
			aria-label="Settings"
		>
			{#each SECTIONS as section (section.slug)}
				<!-- The active tab keeps its look under the pointer. -->
				<a
					class={[
						'flex items-center gap-2 rounded-sm px-2 py-1 whitespace-nowrap hover:no-underline max-md:rounded-b-none max-md:border-b-2 max-md:border-transparent',
						current === section.slug
							? 'bg-current-item font-medium text-fg max-md:border-b-fg max-md:bg-transparent'
							: 'text-fg-muted hover:bg-bg-hover hover:text-fg'
					]}
					href={routes.settings(section.slug)}
					aria-current={current === section.slug ? 'page' : undefined}
				>
					{section.label}
				</a>
			{/each}
		</nav>
		<!-- Moving between sections fades only this column (see the root layout). -->
		<div class="min-w-0" data-view-page="/settings">
			{@render children()}
		</div>
	</div>
</div>
