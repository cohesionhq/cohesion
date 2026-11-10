<!--
	An article's panel (`ItemPanel`): its feed, who wrote it and when it was
	published, then its text, with Open going to its page in the current
	workspace.
-->
<script lang="ts">
	import CalendarDays from '@lucide/svelte/icons/calendar-days';
	import Newspaper from '@lucide/svelte/icons/newspaper';
	import Rss from '@lucide/svelte/icons/rss';
	import UserPen from '@lucide/svelte/icons/user-pen';
	import type { Article } from '$lib/protocol/types';
	import { articleRoute } from '$lib/shell/links';
	import ItemDetail from '$lib/ui/ItemDetail.svelte';
	import ItemPanel from '$lib/ui/ItemPanel.svelte';
	import Markdown from '$lib/ui/Markdown.svelte';
	import { publishedDate } from './published';

	let {
		open = $bindable(false),
		article,
		feedTitle
	}: { open?: boolean; article: Article; feedTitle: string } = $props();

	let href = $state<string>();

	$effect(() => {
		const id = article.id;
		void articleRoute(id).then((route) => (href = route ?? undefined));
	});
</script>

<ItemPanel
	bind:open
	title={article.title}
	icon={Newspaper}
	kind="Article"
	{href}
	link={{ kind: 'article', ws: null, id: article.id }}
>
	{#snippet details()}
		<ItemDetail label="Feed" icon={Rss}>{feedTitle}</ItemDetail>
		{#if article.author}
			<ItemDetail label="Author" icon={UserPen}>{article.author}</ItemDetail>
		{/if}
		<ItemDetail label="Published" icon={CalendarDays}>{publishedDate(article)}</ItemDetail>
	{/snippet}
	{#if article.contents.trim()}
		<div class="font-serif text-md"><Markdown source={article.contents} /></div>
	{:else}
		<p class="text-fg-subtle">Its page hasn’t been fetched.</p>
	{/if}
</ItemPanel>
