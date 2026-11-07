<!--
	The Tasks page's filters, on one line under its header: the Filters button,
	which counts everything narrowing the list and opens a panel of every filter,
	then the priorities and sizes kept, then the tags, which say which tasks these
	are. A line too long for the width scrolls sideways; on a phone it keeps only
	the tags in use, the rest being in the panel.
-->
<script lang="ts">
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import Flag from '@lucide/svelte/icons/flag';
	import type { MouseEventHandler } from 'svelte/elements';
	import { filterRowClass } from '$lib/features/tags/TagFilter.svelte';
	import FilterPill, { pillClass, type PillState } from '$lib/features/tags/FilterPill.svelte';
	import type { Tag, TaskPriority, TaskSize } from '$lib/protocol/types';
	import { pane } from '$lib/shell/pane.svelte';
	import Popover from '$lib/ui/Popover.svelte';
	import { priorities as allPriorities } from './priority';
	import { sizes as allSizes } from './size';

	interface Props {
		tags: Tag[];
		tagState: (tag_id: number) => PillState;
		tagHref: (tag_id: number) => string;
		/** The priorities the list keeps. */
		priorities: TaskPriority[];
		priorityHref: (priority: TaskPriority) => string;
		/** The sizes the list keeps. */
		sizes: TaskSize[];
		sizeHref: (size: TaskSize) => string;
		onmanagetags: () => void;
	}

	let { tags, tagState, tagHref, priorities, priorityHref, sizes, sizeHref, onmanagetags }: Props =
		$props();

	const page = pane();

	let open = $state(false);

	const active = $derived(
		priorities.length + sizes.length + tags.filter((t) => tagState(t.id) !== null).length
	);

	// The panel is portalled out of its half, so it moves the half itself, and stays open.
	const pick: MouseEventHandler<HTMLAnchorElement> = (event) => {
		if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
		event.preventDefault();
		const { pathname, search } = event.currentTarget;
		void page.goto(pathname + search, { keepFocus: true });
	};

	const groupLabel = 'text-xs font-medium text-fg-subtle';
	const groupPills = 'flex flex-wrap items-center gap-1.5';
</script>

{#snippet priorityPill(p: ReturnType<typeof allPriorities>[number], onclick?: typeof pick)}
	{@const on = priorities.includes(p.value)}
	<FilterPill label={p.label} filter={on ? 'with' : null} href={priorityHref(p.value)} {onclick}>
		{#snippet icon()}
			{#if p.value !== 'none'}<Flag
					class={on ? undefined : p.tone}
					size={12}
					strokeWidth={1.75}
					fill="currentColor"
					aria-hidden="true"
				/>{/if}
		{/snippet}
	</FilterPill>
{/snippet}

{#snippet sizePill(s: ReturnType<typeof allSizes>[number], onclick?: typeof pick)}
	<FilterPill
		label={s.label}
		filter={sizes.includes(s.value) ? 'with' : null}
		href={sizeHref(s.value)}
		{onclick}
	/>
{/snippet}

<div class="mb-4 flex items-center gap-1.5">
	<Popover bind:open>
		{#snippet trigger(props)}
			<button
				class={[pillClass(active > 0 ? 'with' : null), 'flex-none cursor-pointer']}
				type="button"
				aria-label={active > 0 ? `Filters, ${active} active` : 'Filters'}
				{...props}
			>
				Filters{#if active > 0}<span>{active}</span>{/if}
				<ChevronDown class="flex-none" size={14} strokeWidth={1.75} aria-hidden="true" />
			</button>
		{/snippet}
		<div class="flex w-72 max-w-full flex-col gap-3 p-2">
			<nav class="flex flex-col gap-1.5" aria-label="Priority">
				<span class={groupLabel}>Priority</span>
				<div class={groupPills}>
					{#each allPriorities() as p (p.value)}
						{@render priorityPill(p, pick)}
					{/each}
				</div>
			</nav>
			<nav class="flex flex-col gap-1.5" aria-label="Size">
				<span class={groupLabel}>Size</span>
				<div class={groupPills}>
					{#each allSizes() as s (s.value)}
						{@render sizePill(s, pick)}
					{/each}
				</div>
			</nav>
			{#if tags.length > 0}
				<nav class="flex flex-col gap-1.5" aria-label="Tags">
					<span class={groupLabel}>Tags</span>
					<div class={groupPills}>
						{#each tags as tag (tag.id)}
							<FilterPill
								label={tag.name}
								filter={tagState(tag.id)}
								href={tagHref(tag.id)}
								onclick={pick}
							/>
						{/each}
					</div>
				</nav>
				<button
					class="cursor-pointer self-start text-xs whitespace-nowrap text-fg-subtle hover:text-fg"
					type="button"
					onclick={() => {
						open = false;
						onmanagetags();
					}}>Manage tags…</button
				>
			{/if}
		</div>
	</Popover>
	<nav class={[filterRowClass, 'min-w-0 flex-1']} aria-label="Filters in use and tags">
		{#each allPriorities().filter((p) => priorities.includes(p.value)) as p (p.value)}
			{@render priorityPill(p)}
		{/each}
		{#each allSizes().filter((s) => sizes.includes(s.value)) as s (s.value)}
			{@render sizePill(s)}
		{/each}
		{#each tags as tag (tag.id)}
			{@const filter = tagState(tag.id)}
			<FilterPill
				label={tag.name}
				{filter}
				href={tagHref(tag.id)}
				class={filter === null ? 'max-sm:hidden' : undefined}
			/>
		{/each}
	</nav>
</div>
