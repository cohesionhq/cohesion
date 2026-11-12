<!--
	The Documents and Tasks pages' heading: breadcrumbs from the page down through the folders
	holding the one shown, each a link, then the title of what's shown as a button opening the
	switcher (`children`), where every folder is listed and managed. Below 600px only the title
	shows.
-->
<script lang="ts" module>
	export interface Crumb {
		label: string;
		href: string;
	}
</script>

<script lang="ts">
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import type { Snippet } from 'svelte';
	import Popover from '$lib/ui/Popover.svelte';

	interface Props {
		/** The way down to what's shown, from the page itself; empty on the page's own list. */
		crumbs: Crumb[];
		/** What's shown: a folder, the Inbox or a list. */
		title: string;
		/** Whether the switcher is open; a pick in it closes it. */
		open?: boolean;
		/** The switcher's contents. */
		children: Snippet;
	}

	let { crumbs, title, open = $bindable(false), children }: Props = $props();
</script>

<div class="flex min-w-0 items-center gap-1">
	{#if crumbs.length > 0}
		<nav class="flex min-w-0 flex-none items-center max-sm:hidden" aria-label="Breadcrumbs">
			<ol class="flex min-w-0 items-center gap-1">
				{#each crumbs as crumb (crumb.href)}
					<li class="flex min-w-0 items-center gap-1">
						<a
							class="max-w-40 truncate text-md leading-tight text-fg-muted no-underline hover:text-fg hover:no-underline"
							href={crumb.href}>{crumb.label}</a
						>
						<ChevronRight
							class="flex-none text-fg-subtle"
							size={14}
							strokeWidth={1.75}
							aria-hidden="true"
						/>
					</li>
				{/each}
			</ol>
		</nav>
	{/if}
	<h1 class="flex min-w-0">
		<Popover bind:open align="start">
			{#snippet trigger(props)}
				<button
					class="-mx-1.5 flex min-w-0 cursor-pointer items-center gap-1 rounded-sm px-1.5 py-0.5 text-md leading-tight font-semibold hover:bg-bg-hover"
					type="button"
					{...props}
				>
					<span class="truncate">{title}</span>
					<span class="sr-only">, choose a folder</span>
					<ChevronDown
						class="flex-none text-fg-subtle"
						size={14}
						strokeWidth={1.75}
						aria-hidden="true"
					/>
				</button>
			{/snippet}
			<div class="flex w-72 max-w-full flex-col">
				{@render children()}
			</div>
		</Popover>
	</h1>
</div>
