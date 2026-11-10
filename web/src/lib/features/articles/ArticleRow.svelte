<!-- One article in a list: its title, who wrote it and where it's from, when it was published
     and whether it's read. -->
<script lang="ts">
	import Ellipsis from '@lucide/svelte/icons/ellipsis';
	import type { ArticleSummary } from '$lib/protocol/types';
	import { copyLinkItems } from '$lib/ui/copyLink';
	import Menu, { type MenuItem } from '$lib/ui/Menu.svelte';
	import { hostOf } from './failure';
	import { publishedAge } from './published';

	interface Props {
		article: Pick<
			ArticleSummary,
			'id' | 'title' | 'url' | 'author' | 'published_at' | 'published_precision' | 'read'
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

	const source = $derived(from === undefined ? hostOf(article.url) : from);
	// A feed whose author is the feed itself (a blog's own name) says it once.
	const place = $derived(source === article.author ? null : source);
	const items = $derived<MenuItem[]>([
		{
			label: article.read ? 'Mark as unread' : 'Mark as read',
			onSelect: () => onread(!article.read)
		},
		...copyLinkItems(() => ({
			link: { kind: 'article', ws: null, id: article.id },
			title: article.title
		}))
	]);

	const tool =
		'inline-flex size-7 flex-none cursor-pointer items-center justify-center rounded-sm text-fg-subtle transition-colors hover:bg-bg-hover hover:text-fg';
</script>

<li class="group/row relative flex items-start gap-2 border-border not-first:border-t">
	<!-- A fixed slot for the unread dot, so every title starts at the same place. -->
	<!-- Centred on the title's line: the row's top padding and half its line, less half the dot. -->
	<span class="flex w-2 flex-none pt-4.5" aria-hidden="true">
		{#if !article.read}<span class="size-1.5 rounded-full bg-accent"></span>{/if}
	</span>
	<a
		class="group flex min-w-0 flex-1 flex-col gap-0.5 py-2.5 text-fg hover:no-underline touch:pr-9"
		{href}
	>
		<span class="flex items-baseline justify-between gap-4">
			<span
				class={[
					'min-w-0 truncate transition-colors group-hover:text-accent',
					article.read ? 'text-fg-muted' : 'font-semibold'
				]}
				>{article.title}{#if !article.read}<span class="sr-only">, unread</span>{/if}</span
			>
			<!-- The age ends the row at the list's edge; the menu takes its place on hover. -->
			<span
				class="flex-none text-xs text-fg-subtle tabular-nums transition-opacity group-focus-within/row:opacity-0 group-hover/row:opacity-0 touch:hidden"
			>
				<!-- A state of the row, not part of its byline: a status of its own before the age, which
				     keeps the form every row's has. -->
				{#if article.snapshot?.state === 'failed'}<span class="mr-3">Not fetched</span
					>{/if}{publishedAge(article, now)}
			</span>
		</span>
		{#if article.author || place}
			<!-- Joined with a spaced dot, as every "Label · time" line in the app is. -->
			<span class="min-w-0 truncate text-xs text-fg-subtle"
				>{[article.author, place].filter(Boolean).join(' · ')}</span
			>
		{/if}
		{#if excerpt}<span class="line-clamp-2 text-xs text-fg-muted">{excerpt}</span>{/if}
	</a>
	<span
		class="absolute top-2 right-0 flex items-center opacity-0 transition-opacity group-focus-within/row:opacity-100 group-hover/row:opacity-100 has-data-[state=open]:opacity-100 touch:opacity-100"
	>
		<Menu {items}>
			{#snippet trigger(props)}
				<button class={tool} type="button" aria-label="More actions for {article.title}" {...props}>
					<Ellipsis size={15} strokeWidth={1.75} />
				</button>
			{/snippet}
		</Menu>
	</span>
</li>
