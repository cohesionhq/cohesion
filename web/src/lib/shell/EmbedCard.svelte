<!--
	A `cohesion://` link alone on its line, shown as a card of the item it
	names (`embeds`): its icon (a task's box, an agent's avatar, a session's
	status), its title, what it is, then the start of its text; a database
	adds its first rows as a small table, and a database row a few of its
	values before its document's text. A click opens
	it over the page, as the link would (`linkedItem.follow`), or goes to a
	database's table. While it
	resolves, the link's own text shimmers; once its item is gone, the text
	stays, quiet, with "Task not found"; if it can't be resolved, it is the
	plain link.
-->
<script lang="ts">
	import type { Component } from 'svelte';
	import { publishedDate } from '$lib/features/articles/published';
	import { cellValue, formatValue, isNumeric } from '$lib/features/databases/values';
	import { longDay, localDay, shortDay } from '$lib/format/dates';
	import type { DatabaseProperty, DatabaseRowEntry, Embed } from '$lib/protocol/types';
	import { vocabulary } from '$lib/protocol/vocabulary.svelte';
	import { connectors } from '$lib/stores/connectors.svelte';
	import { embeds } from '$lib/stores/embeds.svelte';
	import AgentAvatar from '$lib/ui/AgentAvatar.svelte';
	import Shimmer from '$lib/ui/Shimmer.svelte';
	import StatusIcon from '$lib/ui/StatusIcon.svelte';
	import { embedIcon, KIND_ICONS } from './embedIcons';
	import { linkedItem } from './linkedItem.svelte';
	import { itemLinkRoute, parseItemLink, type ItemKind } from './routes';

	interface Props {
		/** The link, `cohesion://task/3/42`. */
		href: string;
		/** The link's text. */
		label: string;
	}

	let { href, label }: Props = $props();

	const link = $derived(parseItemLink(href));
	let shown = $state<ReturnType<typeof embeds.show>>();

	$effect(() => {
		const current = embeds.show(href);
		shown = current;
		return current.release;
	});

	const embed = $derived(shown?.embed.value ?? null);
	const loaded = $derived(shown?.embed.loaded ?? false);
	const failed = $derived(shown?.embed.error !== null && shown?.embed.error !== undefined);

	const MISSING: Record<ItemKind, string> = {
		task: 'Task not found',
		document: 'Document not found',
		resource: 'Resource not found',
		database: 'Database not found',
		'database-row': 'Row not found',
		agent: 'Agent not found',
		'actor-session': 'Session not found',
		article: 'Article not found'
	};

	/** The quiet line under the title: what it is, then a detail or two. */
	function detail(embed: Embed): string {
		const kind = vocabulary.embedKind(embed.kind);
		const { item } = embed;
		switch (item.kind) {
			case 'task': {
				const { task } = item;
				const due = task.due_date ? `Due ${shortDay(task.due_date, localDay())}` : null;
				return [`${kind} #${task.id}`, due].filter(Boolean).join(' · ');
			}
			case 'document':
				return item.document.journal_date
					? `${kind} · ${longDay(item.document.journal_date)}`
					: kind;
			case 'resource':
				return connectors.kindTitle(item.resource);
			case 'agent':
				return kind;
			case 'actorSession':
				return [kind, vocabulary.actorSessionStatus(item.session.status), item.actor_title]
					.filter(Boolean)
					.join(' · ');
			case 'article':
				return `${item.feed_title} · ${publishedDate(item.article)}`;
			case 'database':
				return `${kind} · ${item.row_count === 1 ? '1 row' : `${item.row_count} rows`}`;
			case 'databaseRow':
				return `${kind} · ${item.database_title}`;
		}
	}

	/** The most columns a database's card shows. */
	const PREVIEW_COLUMNS = 5;
	/** The most values a row's card shows. */
	const ROW_VALUES = 4;

	/** A value as a card shows it: a checkbox only when checked, an empty one blank. */
	function shownValue(row: DatabaseRowEntry, property: DatabaseProperty): string {
		const value = cellValue(row, property.id);
		if (value === null || (value.type === 'checkbox' && !value.value)) return '';
		return formatValue(value);
	}

	/** A row's values worth showing on its card: those it holds, but the one its label is. */
	function rowValues(row: DatabaseRowEntry, properties: DatabaseProperty[]) {
		const labelled = properties.find((p) => p.data_type === 'text');
		return properties
			.filter((p) => p.id !== labelled?.id)
			.map((p) => ({ property: p, text: shownValue(row, p) }))
			.filter((v) => v.text !== '')
			.slice(0, ROW_VALUES);
	}

	const card =
		'flex w-full min-w-0 items-start gap-3 overflow-hidden rounded-md border border-border bg-bg-raised px-3 py-2.5 text-sm text-fg no-underline transition-colors hover:bg-bg-hover hover:no-underline';
</script>

{#snippet tile(Icon: Component)}
	<span
		class="inline-flex size-7 flex-none items-center justify-center rounded-sm border border-border bg-bg text-fg-muted"
	>
		<Icon size={15} strokeWidth={1.75} aria-hidden="true" />
	</span>
{/snippet}

{#if link === null || (failed && embed === null)}
	<a class="text-accent" href={link ? itemLinkRoute(link) : href}>{label}</a>
{:else}
	<a class={card} href={itemLinkRoute(link)} onclick={(event) => linkedItem.follow(event, link)}>
		{#if embed}
			{#if embed.item.kind === 'agent'}
				<AgentAvatar
					name={embed.item.agent.agent_name}
					color={embed.item.agent.agent_color}
					emoji={embed.item.agent.agent_emoji}
					size="sm"
				/>
			{:else if embed.item.kind === 'actorSession'}
				<span class="inline-flex size-7 flex-none items-center justify-center">
					<StatusIcon status={embed.item.session.status} unread={embed.item.session.unread} />
				</span>
			{:else}
				{@render tile(embedIcon(embed))}
			{/if}
			<span class="flex min-w-0 flex-1 flex-col gap-0.5">
				<span class="flex min-w-0 items-baseline gap-2">
					<span
						class={[
							'min-w-0 truncate font-medium',
							embed.item.kind === 'task' &&
								embed.item.task.status !== 'incomplete' &&
								'text-fg-muted line-through'
						]}>{embed.title || label}</span
					>
					<span class="min-w-0 shrink-[2] truncate text-xs text-fg-subtle">{detail(embed)}</span>
				</span>
				{#if embed.item.kind === 'databaseRow'}
					{@const values = rowValues(embed.item.row, embed.item.properties)}
					{#if values.length}
						<span class="flex min-w-0 flex-wrap gap-x-4 gap-y-0.5 text-xs">
							{#each values as { property, text } (property.id)}
								<span class="flex min-w-0 gap-1.5">
									<span class="flex-none text-fg-subtle">{property.name}</span>
									<span class="min-w-0 truncate text-fg-muted">{text}</span>
								</span>
							{/each}
						</span>
					{/if}
				{/if}
				{#if embed.excerpt}
					<span class="line-clamp-2 text-xs text-fg-muted">{embed.excerpt}</span>
				{/if}
				{#if embed.item.kind === 'database' && embed.item.properties.length && embed.item.rows.length}
					{@const columns = embed.item.properties.slice(0, PREVIEW_COLUMNS)}
					<span class="mt-1.5 block min-w-0 overflow-hidden">
						<table class="w-full table-fixed border-collapse text-xs">
							<thead>
								<tr class="border-b border-border">
									{#each columns as property (property.id)}
										<th
											class={[
												'truncate px-1.5 py-1 font-medium text-fg-subtle first:pl-0',
												isNumeric(property.data_type) ? 'text-right' : 'text-left'
											]}>{property.name}</th
										>
									{/each}
								</tr>
							</thead>
							<tbody>
								{#each embed.item.rows as row (row.id)}
									<tr class="border-b border-border last:border-b-0">
										{#each columns as property (property.id)}
											<td
												class={[
													'truncate px-1.5 py-1 text-fg-muted first:pl-0',
													isNumeric(property.data_type) && 'text-right tabular-nums'
												]}>{shownValue(row, property)}</td
											>
										{/each}
									</tr>
								{/each}
							</tbody>
						</table>
					</span>
				{/if}
			</span>
		{:else if loaded}
			{@render tile(KIND_ICONS[link.kind])}
			<span class="flex min-w-0 flex-1 items-baseline gap-2 self-center">
				<span class="min-w-0 truncate text-fg-muted">{label}</span>
				<span class="min-w-0 shrink-[2] truncate text-xs text-fg-subtle">{MISSING[link.kind]}</span>
			</span>
		{:else}
			{@render tile(KIND_ICONS[link.kind])}
			<Shimmer active class="min-w-0 flex-1 self-center truncate">{label}</Shimmer>
		{/if}
	</a>
{/if}
