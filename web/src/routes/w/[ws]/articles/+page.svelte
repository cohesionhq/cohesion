<script lang="ts">
	import ArrowUpRight from '@lucide/svelte/icons/arrow-up-right';
	import Ellipsis from '@lucide/svelte/icons/ellipsis';
	import RefreshCw from '@lucide/svelte/icons/refresh-cw';
	import { untrack } from 'svelte';
	import { ago, nowSeconds } from '$lib/format/dates';
	import AddFeedDialog from '$lib/features/articles/AddFeedDialog.svelte';
	import ArticleRow from '$lib/features/articles/ArticleRow.svelte';
	import CleanUpDialog from '$lib/features/articles/CleanUpDialog.svelte';
	import { failureSentence } from '$lib/features/articles/failure';
	import { markAllRead, markRead } from '$lib/features/articles/read';
	import FolderNav from '$lib/features/folders/FolderNav.svelte';
	import {
		navAction,
		navCount,
		navHeading,
		navItem,
		navLink,
		navPane
	} from '$lib/features/folders/nav';
	import { toProtocolError } from '$lib/protocol/errors';
	import type { Feed } from '$lib/protocol/types';
	import { workspaceLoadError } from '$lib/shell/loadError';
	import { routes } from '$lib/shell/routes';
	import {
		parseView,
		rememberView,
		rememberedView,
		sameView,
		viewParam,
		type ArticleView
	} from '$lib/stores/articles.svelte';
	import { feeds } from '$lib/stores/feeds.svelte';
	import Button from '$lib/ui/Button.svelte';
	import ConfirmDialog from '$lib/ui/ConfirmDialog.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import EmptyState from '$lib/ui/EmptyState.svelte';
	import Field from '$lib/ui/Field.svelte';
	import Menu, { type MenuItem } from '$lib/ui/Menu.svelte';
	import PageHeader from '$lib/ui/PageHeader.svelte';
	import Pager, { rememberedPageSize } from '$lib/ui/Pager.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';
	import { pane } from '$lib/shell/pane.svelte';
	import PageTitle from '$lib/shell/PageTitle.svelte';
	import { pageStores } from '$lib/stores/pageStores.svelte';

	const { articles } = pageStores();

	const page = pane();

	const ws = $derived(Number(page.params.ws));
	const param = $derived(page.url.searchParams.get('view'));
	// Without `?view=` (the sidebar's link, say), the view last shown here.
	const view = $derived<ArticleView>(param !== null ? parseView(param) : rememberedView());

	$effect(() => rememberView(view));

	// `?page=` counts from 1; a view opens on its first.
	let limit = $state(rememberedPageSize());
	const offset = $derived(
		Math.max(0, Number(page.url.searchParams.get('page') ?? 1) - 1 || 0) * limit
	);

	function showPage(next: number) {
		const url = new URL(page.url);
		if (next === 0) url.searchParams.delete('page');
		else url.searchParams.set('page', String(next / limit + 1));
		void page.goto(`${url.pathname}${url.search}`, { keepFocus: true });
	}

	function showSize(size: number) {
		limit = size;
		showPage(0);
	}

	$effect(() => {
		const [v, o, l] = [view, offset, limit];
		untrack(() => {
			void articles.open(v, o, l);
			void feeds.load();
		});
	});

	// A page past the end (its last articles just went) shows the last one there is.
	$effect(() => {
		const { articles: listed, total } = articles.list.value;
		if (articles.list.loaded && listed.length === 0 && total > 0 && offset > 0)
			untrack(() => showPage(Math.floor((total - 1) / limit) * limit));
	});

	// A new view starts with no filter; another page of the same one keeps it.
	$effect(() => {
		void view;
		query = '';
	});

	const href = (v: ArticleView) => routes.articles(ws, viewParam(v));

	const feed = $derived(
		typeof view === 'object' ? (feeds.list.value.find((f) => f.id === view.feed) ?? null) : null
	);
	const gone = $derived(typeof view === 'object' && feeds.list.loaded && feed === null);
	const viewName = $derived(view === 'feeds' ? 'All feeds' : (feed?.title ?? 'Articles'));

	// ---- Filtering ----------------------------------------------------------

	let query = $state('');
	const shown = $derived.by(() => {
		const q = query.trim().toLowerCase();
		if (q === '') return articles.list.value.articles;
		return articles.list.value.articles.filter(
			(l) =>
				l.title.toLowerCase().includes(q) ||
				l.url.toLowerCase().includes(q) ||
				(l.author ?? '').toLowerCase().includes(q)
		);
	});
	const now = nowSeconds();

	// ---- Dialogs the palette opens -------------------------------------------

	let addingFeed = $state(false);

	// The palette's "Add feed…" lands here with `?add=feed`.
	$effect(() => {
		const params = page.url.searchParams;
		if (params.get('add') !== 'feed') return;
		untrack(() => {
			addingFeed = true;
			const url = new URL(page.url);
			url.searchParams.delete('add');
			void page.goto(`${url.pathname}${url.search}`, { replaceState: true, keepFocus: true });
		});
	});

	// ---- Rows ---------------------------------------------------------------

	function run(promise: Promise<unknown>) {
		promise.catch((error) => toasts.error(error));
	}

	// ---- Read state ---------------------------------------------------------

	// Only once the list shows this view: marking acts on the loaded one.
	const unread = $derived(
		!gone &&
			articles.list.loaded &&
			sameView(articles.list.key!.view, view) &&
			// Other pages may hold unread ones.
			(articles.list.value.total > articles.list.value.articles.length ||
				articles.list.value.articles.some((l) => !l.read))
	);

	async function markViewRead() {
		try {
			const marked = await markAllRead(articles);
			toasts.show(
				marked === 1 ? 'Marked 1 article as read.' : `Marked ${marked} articles as read.`
			);
		} catch (error) {
			toasts.error(error);
		}
	}

	const markAllItem = $derived<MenuItem>({
		label: 'Mark all as read',
		disabled: !unread,
		onSelect: markViewRead
	});

	// ---- Feed ---------------------------------------------------------------

	let refreshing = $state(false);

	async function refresh(target: Feed) {
		refreshing = true;
		try {
			const read = await feeds.refresh(target.id);
			if (read.status.state === 'failed')
				toasts.show(
					`Couldn’t read “${read.title}”. ${failureSentence(read.status.failure)}`,
					'error'
				);
			else toasts.show(`Refreshed “${read.title}”.`);
		} catch (error) {
			toasts.error(error);
		} finally {
			refreshing = false;
		}
	}

	async function refreshEvery() {
		refreshing = true;
		try {
			const read = await feeds.refreshAll();
			const failed = read.filter((f) => f.status.state === 'failed').length;
			if (failed > 0) toasts.show(`Couldn’t read ${failed} of ${read.length} feeds.`, 'error');
			else toasts.show(`Refreshed ${read.length} ${read.length === 1 ? 'feed' : 'feeds'}.`);
		} catch (error) {
			toasts.error(error);
		} finally {
			refreshing = false;
		}
	}

	function feedStatus(f: Feed): string {
		if (f.status.state === 'failed')
			return `Couldn’t read it ${ago(f.status.at)}. ${failureSentence(f.status.failure)}`;
		return `Read ${ago(f.status.at)}`;
	}

	let cleaningUp = $state(false);
	let deletingFeed = $state<Feed | null>(null);

	async function removeFeed(target: Feed) {
		await feeds.remove(target.id);
		void page.goto(href('feeds'), { replaceState: true });
	}

	// ---- Renaming a feed ------------------------------------------------------

	let renaming = $state<Feed | null>(null);
	let renameOpen = $state(false);
	let renameTitle = $state('');
	let renameError = $state<string | null>(null);
	let renameBusy = $state(false);

	function openRename(next: Feed) {
		renaming = next;
		renameTitle = next.title;
		renameError = null;
		renameOpen = true;
	}

	async function submitRename(event: SubmitEvent) {
		event.preventDefault();
		const title = renameTitle.trim();
		if (renaming === null || title === '') return;
		renameBusy = true;
		try {
			await feeds.rename(renaming.id, title);
			renameOpen = false;
		} catch (error) {
			const e = toProtocolError(error);
			if (e.code === 'validationFailed') renameError = e.message;
			else toasts.error(e);
		} finally {
			renameBusy = false;
		}
	}

	const feedById = (id: number) => feeds.list.value.find((f) => f.id === id);
	const feedTitles = $derived(new Map(feeds.list.value.map((f) => [f.id, f.title])));
	const tool =
		'inline-flex size-7.5 cursor-pointer items-center justify-center rounded-sm text-fg-muted transition-colors hover:bg-bg-hover hover:text-fg';
</script>

<PageTitle title={`${viewName} · Articles`} />

<div class="flex gap-6 px-(--page-inset) max-md:flex-col max-md:gap-0">
	<aside class={navPane}>
		<FolderNav
			label="Feeds"
			newLabel="Add feed…"
			folders={feeds.list.value}
			current={feed?.id ?? null}
			href={(id) => href({ feed: id })}
			unread={(f) => feedById(f.id)?.unread ?? 0}
			onnew={() => (addingFeed = true)}
			onreorder={(ids) => feeds.reorder(ids).catch((e) => toasts.error(e))}
		>
			{#snippet before()}
				<li class={navHeading}>Feeds</li>
				<li class={navItem}>
					<a
						class={navLink}
						href={href('feeds')}
						aria-current={view === 'feeds' ? 'page' : undefined}
						>All feeds{#if feeds.unread > 0}<span class="sr-only">, {feeds.unread} unread</span
							>{/if}</a
					>
					{#if feeds.unread > 0}
						<span class={navCount} aria-hidden="true">{feeds.unread}</span>
					{/if}
				</li>
			{/snippet}
			{#snippet actions()}
				<!-- Research is a page of its own, not a view of this list. -->
				<a
					class={[navAction, 'inline-flex items-center gap-1 hover:no-underline']}
					href={routes.research(ws)}
					>Research <ArrowUpRight size={12} strokeWidth={1.75} aria-hidden="true" /></a
				>
			{/snippet}
		</FolderNav>
	</aside>

	<!-- The header beside the nav isn't where other pages' headers are, so it fades
	     with the page rather than swapping in place (see tokens.css). Picking a view
	     fades this column alone (see the root layout). -->
	<div class="min-w-0 flex-1 [&>header]:[view-transition-name:none]" data-view-query="view">
		<PageHeader title={viewName}>
			{#snippet meta()}
				{#if feed}<span>{feedStatus(feed)}</span>{/if}
			{/snippet}
			{#snippet actions()}
				{#if feed}
					<button
						class={tool}
						type="button"
						aria-label="Refresh feed"
						disabled={refreshing}
						onclick={() => refresh(feed)}
					>
						<RefreshCw
							class={refreshing ? 'animate-spin motion-reduce:animate-none' : ''}
							size={16}
							strokeWidth={1.75}
						/>
					</button>
					<Menu
						items={[
							markAllItem,
							{
								label: 'Rename…',
								separated: true,
								onSelect: () => openRename(feed)
							},
							{
								label: 'Fetch whole pages when the feed has excerpts',
								checked: feed.full_page_policy === 'whenExcerpt',
								separated: true,
								onSelect: () => run(feeds.setFullPagePolicy(feed.id, 'whenExcerpt'))
							},
							{
								label: 'Always fetch whole pages',
								checked: feed.full_page_policy === 'always',
								onSelect: () => run(feeds.setFullPagePolicy(feed.id, 'always'))
							},
							{ label: 'Clean up…', separated: true, onSelect: () => (cleaningUp = true) },
							{ label: 'Remove…', danger: true, onSelect: () => (deletingFeed = feed) }
						]}
					>
						{#snippet trigger(props)}
							<button class={tool} type="button" aria-label="More actions" {...props}>
								<Ellipsis size={16} strokeWidth={1.75} />
							</button>
						{/snippet}
					</Menu>
				{:else}
					<button
						class={tool}
						type="button"
						aria-label="Refresh all feeds"
						disabled={refreshing}
						onclick={() => refreshEvery()}
					>
						<RefreshCw
							class={refreshing ? 'animate-spin motion-reduce:animate-none' : ''}
							size={16}
							strokeWidth={1.75}
						/>
					</button>
					<Menu
						items={[
							markAllItem,
							{ label: 'Clean up…', separated: true, onSelect: () => (cleaningUp = true) }
						]}
					>
						{#snippet trigger(props)}
							<button class={tool} type="button" aria-label="More actions" {...props}>
								<Ellipsis size={16} strokeWidth={1.75} />
							</button>
						{/snippet}
					</Menu>
				{/if}
			{/snippet}
		</PageHeader>

		{#if gone}
			<EmptyState
				message="Feed not found"
				actionLabel="Back to all feeds"
				onaction={() => page.goto(href('feeds'), { replaceState: true })}
			/>
		{:else if articles.list.error && !articles.list.loaded}
			<EmptyState {...workspaceLoadError(articles.list.error, () => articles.list.refetch())} />
		{:else if articles.list.loaded && sameView(articles.list.key!.view, view)}
			{#if articles.list.value.total === 0}
				{#if feed}
					<EmptyState compact message="No articles from this feed yet" />
				{:else}
					<EmptyState
						message="No articles from your feeds yet"
						actionLabel="Add feed…"
						onaction={() => (addingFeed = true)}
					/>
				{/if}
			{:else}
				<div class="mb-5 max-w-80">
					<TextInput
						bind:value={query}
						type="search"
						placeholder="Filter by title, author or address"
						aria-label="Filter articles by title, author or address"
						autocomplete="off"
					/>
				</div>

				{#if shown.length === 0}
					<p class="text-fg-muted">No matches.</p>
				{:else}
					<ul aria-label="{viewName} articles">
						{#each shown as article (article.id)}
							<ArticleRow
								{article}
								href={routes.article(ws, article.id)}
								{now}
								from={feed ? null : feedTitles.get(article.feed_id)}
								onread={(read) => run(markRead(articles, article.id, read))}
							/>
						{/each}
					</ul>
				{/if}
				<Pager
					{offset}
					{limit}
					total={articles.list.value.total}
					onpage={showPage}
					onsize={showSize}
				/>
			{/if}
		{/if}
	</div>
</div>

<AddFeedDialog bind:open={addingFeed} workspaceId={ws} />
<CleanUpDialog bind:open={cleaningUp} feeds={feeds.list.value} initial={feed ? [feed.id] : []} />
<ConfirmDialog
	subject={deletingFeed}
	onclose={() => (deletingFeed = null)}
	title="Remove “{deletingFeed?.title ?? ''}”?"
	confirmLabel="Remove"
	description="Articles from this feed will also be removed."
	onconfirm={removeFeed}
/>

<Dialog bind:open={renameOpen} title="Rename feed">
	<form id="feed-rename-form" onsubmit={submitRename}>
		<Field label="Title" for="feed-rename-title" error={renameError}>
			<TextInput
				id="feed-rename-title"
				bind:value={renameTitle}
				autocomplete="off"
				oninput={() => (renameError = null)}
			/>
		</Field>
	</form>
	{#snippet footer()}
		<Button onclick={() => (renameOpen = false)}>Cancel</Button>
		<Button
			variant="primary"
			type="submit"
			form="feed-rename-form"
			disabled={renameBusy || renameTitle.trim() === ''}
		>
			Rename
		</Button>
	{/snippet}
</Dialog>
