<!-- Research: search everything the feeds brought in, with every article listed until a search. -->
<script lang="ts">
	import { untrack } from 'svelte';
	import { nowSeconds } from '$lib/format/dates';
	import { prefixQuery, SEARCH_DEBOUNCE_MS } from '$lib/format/search';
	import ArticleRow from '$lib/features/articles/ArticleRow.svelte';
	import { markRead } from '$lib/features/articles/read';
	import { toProtocolError } from '$lib/protocol/errors';
	import type { FeedCorpusHit } from '$lib/protocol/types';
	import { workspaceLoadError } from '$lib/shell/loadError';
	import { routes } from '$lib/shell/routes';
	import { feeds } from '$lib/stores/feeds.svelte';
	import EmptyState from '$lib/ui/EmptyState.svelte';
	import PageHeader from '$lib/ui/PageHeader.svelte';
	import Pager, { rememberedPageSize } from '$lib/ui/Pager.svelte';
	import Select from '$lib/ui/Select.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';
	import { pane } from '$lib/shell/pane.svelte';
	import PageTitle from '$lib/shell/PageTitle.svelte';
	import { pageStores } from '$lib/stores/pageStores.svelte';

	const { articles } = pageStores();

	const page = pane();

	const ws = $derived(Number(page.params.ws));
	const now = nowSeconds();

	let limit = $state(rememberedPageSize());
	// The list's page; the matches keep their own.
	let offset = $state(0);

	$effect(() => {
		const [o, l] = [offset, limit];
		untrack(() => {
			void articles.open('feeds', o, l);
			void feeds.load();
		});
	});

	function showSize(size: number) {
		limit = size;
		offset = 0;
		hitOffset = 0;
	}

	const feedTitles = $derived(new Map(feeds.list.value.map((f) => [f.id, f.title])));

	// ---- Searching ----------------------------------------------------------

	// The search lives in `?q=` and `?feed=`, so going back to this page, or
	// reloading it, shows it again.
	let query = $state(page.url.searchParams.get('q') ?? '');
	let feedFilter = $state(page.url.searchParams.get('feed') ?? 'all');
	let hits = $state<FeedCorpusHit[] | null>(null);
	let hitTotal = $state(0);
	let hitOffset = $state(0);
	let searchError = $state<string | null>(null);
	let generation = 0;

	// A new search starts on its first page.
	$effect(() => {
		void [query, feedFilter];
		hitOffset = 0;
	});

	// Typing replaces this page's entry rather than adding one per keystroke.
	$effect(() => {
		const [q, filter] = [query.trim(), feedFilter];
		const timer = setTimeout(() => {
			const url = new URL(untrack(() => page.url));
			if (q === '') url.searchParams.delete('q');
			else url.searchParams.set('q', q);
			if (filter === 'all') url.searchParams.delete('feed');
			else url.searchParams.set('feed', filter);
			if (url.search === untrack(() => page.url.search)) return;
			void page.goto(`${url.pathname}${url.search}`, { replaceState: true, keepFocus: true });
		}, SEARCH_DEBOUNCE_MS);
		return () => clearTimeout(timer);
	});

	// Live search, debounced; a newer query drops older answers.
	$effect(() => {
		const [fts, filter, o, l] = [prefixQuery(query), feedFilter, hitOffset, limit];
		const current = ++generation;
		if (fts === null) {
			hits = null;
			searchError = null;
			return;
		}
		const timer = setTimeout(async () => {
			try {
				const found = await feeds.search(
					fts,
					filter === 'all' ? undefined : [Number(filter)],
					o,
					l
				);
				if (current !== generation) return;
				hits = found.hits;
				hitTotal = found.total;
				searchError = null;
			} catch (error) {
				if (current === generation) searchError = toProtocolError(error).message;
			}
		}, SEARCH_DEBOUNCE_MS);
		return () => clearTimeout(timer);
	});

	// ---- Rows ---------------------------------------------------------------

	async function setRead(id: number, read: boolean) {
		try {
			await markRead(articles, id, read);
			hits = hits?.map((h) => (h.id === id ? { ...h, read } : h)) ?? null;
		} catch (error) {
			toasts.error(error);
		}
	}

	const feedOptions = $derived([
		{ value: 'all', label: 'Every feed' },
		...feeds.list.value.map((f) => ({ value: String(f.id), label: f.title }))
	]);
</script>

<PageTitle title="Research · Articles" />

<div class="px-(--page-inset)">
	<PageHeader title="Research" back={{ href: routes.articles(ws), label: 'Articles' }} />

	{#if feeds.list.loaded && feeds.list.value.length === 0}
		<EmptyState
			message="No feeds yet"
			actionLabel="Add feed…"
			onaction={() => page.goto(routes.addFeed(ws))}
		/>
	{:else}
		<div class="mb-6 flex flex-wrap items-center gap-2">
			<div class="min-w-60 flex-1">
				<TextInput
					bind:value={query}
					type="search"
					placeholder="Search every article from your feeds"
					aria-label="Search every article from your feeds"
					autocomplete="off"
				/>
			</div>
			<Select bind:value={feedFilter} options={feedOptions} aria-label="Feed" />
		</div>

		{#if searchError}
			<p class="text-danger">{searchError}</p>
		{:else if hits !== null}
			<h2 class="mb-2 text-sm font-semibold">Matches</h2>
			{#if hits.length === 0}
				<p class="text-fg-muted">No matches.</p>
			{:else}
				<ul aria-label="Matches">
					{#each hits as hit (hit.id)}
						<ArticleRow
							article={hit}
							href={routes.article(ws, hit.id)}
							{now}
							from={feedTitles.get(hit.feed_id)}
							excerpt={hit.excerpt}
							onread={(read) => setRead(hit.id, read)}
						/>
					{/each}
				</ul>
				<Pager
					offset={hitOffset}
					{limit}
					total={hitTotal}
					onpage={(o) => (hitOffset = o)}
					onsize={showSize}
				/>
			{/if}
		{:else}
			<h2 class="mb-2 text-sm font-semibold">Every article</h2>
			{#if articles.list.error && !articles.list.loaded}
				<EmptyState {...workspaceLoadError(articles.list.error, () => articles.list.refetch())} />
			{:else if articles.list.loaded}
				{#if articles.list.value.total === 0}
					<p class="text-fg-muted">No articles from your feeds yet.</p>
				{:else}
					<ul aria-label="Every article">
						{#each articles.list.value.articles as article (article.id)}
							<ArticleRow
								{article}
								href={routes.article(ws, article.id)}
								{now}
								from={feedTitles.get(article.feed_id)}
								onread={(read) => setRead(article.id, read)}
							/>
						{/each}
					</ul>
					<Pager
						{offset}
						{limit}
						total={articles.list.value.total}
						onpage={(o) => (offset = o)}
						onsize={showSize}
					/>
				{/if}
			{/if}
		{/if}
	{/if}
</div>
