<!--
	Rewind's panel for one item (`ItemPanel`): its versions, newest first, each
	its date and time and who edited it last. Choosing one previews what it
	holds, with Restore unless it is what the item holds now; restoring keeps a
	new version, so the list that comes back can undo it. Every kind shows the
	same way but for its fields.
-->
<script lang="ts">
	import History from '@lucide/svelte/icons/history';
	import { untrack } from 'svelte';
	import { STATUSES } from '$lib/features/tasks/status';
	import { ago, shortDate, shortTime } from '$lib/format/dates';
	import { vocabulary } from '$lib/protocol/vocabulary.svelte';
	import { pageStores } from '$lib/stores/pageStores.svelte';
	import {
		authorLabel,
		type Restored,
		type RewindItem,
		type VersionRow,
		type VersionText
	} from '$lib/stores/rewind.svelte';
	import { storage } from '$lib/stores/settings/storage.svelte';
	import Button from '$lib/ui/Button.svelte';
	import EmptyState from '$lib/ui/EmptyState.svelte';
	import ItemPanel from '$lib/ui/ItemPanel.svelte';
	import { list } from '$lib/ui/list';
	import Markdown from '$lib/ui/Markdown.svelte';
	import ResourceState from '$lib/ui/ResourceState.svelte';
	import AuthorIcon from './AuthorIcon.svelte';
	import Timeline from './Timeline.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';

	interface Props {
		open?: boolean;
		/** What the history is of. */
		item: RewindItem;
		/** The panel's title: the item's name. */
		title: string;
		onrestored?: (restored: Restored) => void;
	}

	let { open = $bindable(false), item, title, onrestored }: Props = $props();

	const { rewind } = pageStores();

	let chosen = $state<VersionRow | null>(null);
	let preview = $state<VersionText | null>(null);
	let restoring = $state(false);

	$effect(() => {
		if (!open) return;
		const showing = item;
		untrack(() => {
			chosen = null;
			preview = null;
			void rewind.versions.load(showing);
			void storage.settings.load();
		});
		return () => {
			rewind.versions.release();
			storage.settings.release();
		};
	});

	const days = $derived(storage.settings.value?.rewind_retention_days);

	const when = (row: VersionRow) => `${shortDate(row.modified)}, ${shortTime(row.modified)}`;

	/** What a row says beside its author: every kind of change but an edit, the usual one. */
	function changeLabel(row: VersionRow): string | null {
		return row.change === 'edited'
			? null
			: (vocabulary.value.version_changes.find((c) => c.kind === row.change)?.label ?? null);
	}

	async function choose(row: VersionRow) {
		chosen = row;
		preview = null;
		try {
			const text = await rewind.read(item, row.id);
			if (chosen?.id === row.id) preview = text;
		} catch (error) {
			chosen = null;
			toasts.error(error);
		}
	}

	async function restore() {
		if (!chosen) return;
		restoring = true;
		try {
			const restored = await rewind.restore(item, chosen.id);
			toasts.show(`Restored the version from ${when(chosen)}.`);
			chosen = null;
			preview = null;
			onrestored?.(restored);
		} catch (error) {
			toasts.error(error);
		} finally {
			restoring = false;
		}
	}
</script>

<ItemPanel bind:open {title} icon={History} kind="Rewind">
	{#snippet actions()}
		{#if chosen}
			<Button onclick={() => (chosen = null)}>Back</Button>
			{#if !chosen.current}
				<Button variant="primary" disabled={!preview || restoring} onclick={restore}>Restore</Button
				>
			{/if}
		{/if}
	{/snippet}
	{#if chosen}
		<p class="mb-3 text-xs text-fg-subtle">
			{when(chosen)}{#if authorLabel(chosen)}&nbsp;· {authorLabel(chosen)}{/if}
		</p>
		{#if preview?.item.kind === 'task'}
			<h3 class="mb-1 text-md font-semibold wrap-anywhere">{preview.title}</h3>
			<p class="mb-4 text-xs text-fg-muted">{STATUSES[preview.item.status]}</p>
			{#if preview.body.trim()}
				<div class="text-md"><Markdown source={preview.body} diagrams /></div>
			{:else}
				<p class="text-fg-subtle">No description.</p>
			{/if}
		{:else if preview?.item.kind === 'document'}
			<h3 class="mb-4 text-md font-semibold wrap-anywhere">{preview.title}</h3>
			{#if preview.body.trim()}
				<div class="text-md"><Markdown source={preview.body} diagrams /></div>
			{:else}
				<p class="text-fg-subtle">Nothing written yet.</p>
			{/if}
		{/if}
	{:else}
		<ResourceState resource={rewind.versions}>
			{#snippet loaded(versions)}
				{#if versions.length === 0}
					<EmptyState message="No versions yet" compact />
				{:else}
					<Timeline
						rows={versions}
						key={(row) => String(row.id)}
						node={(row) => (row.current ? 'current' : row.change === 'deleted' ? 'gone' : 'plain')}
						onchoose={choose}
					>
						{#snippet row(row)}
							<span class="flex items-baseline gap-2">
								<span class="font-medium tabular-nums">{shortTime(row.modified)}</span>
								{#if row.current}
									<span class="rounded-sm bg-bg-hover px-1.5 text-2xs font-medium text-fg">Now</span
									>
								{:else if changeLabel(row)}
									<span class="rounded-sm bg-bg-hover px-1.5 text-2xs text-fg-muted"
										>{changeLabel(row)}</span
									>
								{/if}
								<span class="ml-auto text-xs text-fg-subtle tabular-nums">{ago(row.modified)}</span>
							</span>
							{#if authorLabel(row)}
								<span class="mt-0.5 flex items-center gap-1.5 text-xs text-fg-muted">
									<AuthorIcon author={row.author} />{authorLabel(row)}
								</span>
							{/if}
						{/snippet}
					</Timeline>
				{/if}
				{#if days}
					<p class={list.note}>Versions are kept for {days} {days === 1 ? 'day' : 'days'}.</p>
				{/if}
			{/snippet}
		</ResourceState>
	{/if}
</ItemPanel>
