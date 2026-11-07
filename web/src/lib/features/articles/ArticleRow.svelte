<!-- One article in a list: its title, who wrote it and where it's from, when it was published
     and whether it's read. -->
<script lang="ts">
	import Ellipsis from '@lucide/svelte/icons/ellipsis';
	import type { ArticleSummary } from '$lib/protocol/types';
	import Menu, { type MenuItem } from '$lib/ui/Menu.svelte';
	import { hostOf } from './failure';
	import { publishedAge } from './published';

	interface Props {
		article: Pick<
			ArticleSummary,
			'title' | 'url' | 'author' | 'published_at' | 'published_precision' | 'read'
		> & { snapshot?: ArticleSummary['snapshot'] };
		href: string;
		now: number;
		/**
		 * Where it's from, after the author: its host when absent, its feed's
		 * title in a list of several feeds, `null` in one feed's list.
		 */
		from?: string | null;
		/** A matching passage, under the title. */
		excerpt?: string;
		onread: (read: boolean) => void;
	}

	let { article, href, now, from, excerpt, onread }: Props = $props();

	const place = $derived(from === undefined ? hostOf(article.url) : from);
	const items = $derived<MenuItem[]>([
		{
			label: article.read ? 'Mark as unread' : 'Mark as read',
			onSelect: () => onread(!article.read)
		}
	]);

	const tool =
		'inline-flex size-7 flex-none cursor-pointer items-center justify-center rounded-sm text-fg-subtle transition-colors hover:bg-bg-hover hover:text-fg';
</script>

<li class="group/row flex items-start gap-2 border-border not-first:border-t">
	<a class="group flex min-w-0 flex-1 flex-col gap-0.5 py-2 text-fg hover:no-underline" {href}>
		<span class="relative flex items-baseline justify-between gap-4">
			{#if !article.read}
				<!-- Hangs into the page's inset, so every title starts at the same place. -->
				<span
					class="absolute top-1/2 -left-3 size-1.5 -translate-y-1/2 rounded-full bg-accent"
					aria-hidden="true"
				></span>
			{/if}
			<span
				class={[
					'min-w-0 truncate transition-colors group-hover:text-accent',
					article.read ? 'text-fg-muted' : 'font-semibold'
				]}
				>{article.title}{#if !article.read}<span class="sr-only">, unread</span>{/if}</span
			>
			<span class="flex-none text-xs text-fg-subtle tabular-nums">
				{publishedAge(article, now)}
			</span>
		</span>
		{#if article.author || place || article.snapshot?.state === 'failed'}
			<span class="flex min-w-0 items-baseline gap-2 text-xs text-fg-subtle">
				{#if article.author}<span class="min-w-0 truncate">{article.author}</span>{/if}
				{#if place}<span class="min-w-0 flex-none truncate">{place}</span>{/if}
				{#if article.snapshot?.state === 'failed'}
					<span class="flex-none text-warn">Not fetched</span>
				{/if}
			</span>
		{/if}
		{#if excerpt}<span class="line-clamp-2 text-xs text-fg-muted">{excerpt}</span>{/if}
	</a>
	<span class="flex items-center gap-0.5 pt-1.5">
		<Menu {items}>
			{#snippet trigger(props)}
				<button class={tool} type="button" aria-label="More actions for {article.title}" {...props}>
					<Ellipsis size={15} strokeWidth={1.75} />
				</button>
			{/snippet}
		</Menu>
	</span>
</li>
