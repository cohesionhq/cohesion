<script lang="ts">
	import Ellipsis from '@lucide/svelte/icons/ellipsis';
	import { untrack } from 'svelte';
	import { ago } from '$lib/format/dates';
	import { failureSentence } from '$lib/features/articles/failure';
	import { publishedDate } from '$lib/features/articles/published';
	import { markRead } from '$lib/features/articles/read';
	import { toProtocolError } from '$lib/protocol/errors';
	import { routes } from '$lib/shell/routes';
	import { viewParam } from '$lib/stores/articles.svelte';
	import { feeds } from '$lib/stores/feeds.svelte';
	import Button from '$lib/ui/Button.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import Field from '$lib/ui/Field.svelte';
	import Markdown from '$lib/ui/Markdown.svelte';
	import Menu from '$lib/ui/Menu.svelte';
	import PageHeader from '$lib/ui/PageHeader.svelte';
	import ResourceState from '$lib/ui/ResourceState.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';
	import { copyToClipboard, toasts } from '$lib/ui/toasts.svelte';
	import { pane } from '$lib/shell/pane.svelte';
	import PageTitle from '$lib/shell/PageTitle.svelte';
	import { pageStores } from '$lib/stores/pageStores.svelte';

	const { articles } = pageStores();

	const page = pane();

	const ws = $derived(Number(page.params.ws));
	const id = $derived(Number(page.params.id));
	const article = $derived(articles.current.value);

	let fetching = $state(false);
	let renaming = $state(false);
	let title = $state('');
	let renameError = $state<string | null>(null);
	let renameBusy = $state(false);

	let scroller = $state<HTMLElement>();
	let content = $state<HTMLElement>();
	let progress = $state(0);

	// How far the reader is through the page; 0 while it all fits.
	function measure() {
		if (!scroller) return;
		const max = scroller.scrollHeight - scroller.clientHeight;
		progress = max > 0 ? scroller.scrollTop / max : 0;
	}

	// Images load and the window resizes, so the page's length changes under the reader.
	$effect(() => {
		if (!scroller || !content) return;
		const observer = new ResizeObserver(measure);
		observer.observe(scroller);
		observer.observe(content);
		return () => observer.disconnect();
	});

	$effect(() => {
		const l = id;
		untrack(() => {
			void articles.read(l);
			void feeds.load();
		});
		return () => articles.current.release();
	});

	// Opening an article reads it: mark it read once, so marking it unread again sticks.
	let markedOpen: number | null = null;
	$effect(() => {
		if (!article || article.id !== id || markedOpen === id) return;
		markedOpen = id;
		if (!article.read)
			untrack(() => markRead(articles, article.id, true).catch((e) => toasts.error(e)));
	});

	const feed = $derived(
		article ? feeds.list.value.find((f) => f.id === article.feed_id) : undefined
	);
	// Back to the list the article sits in, when it wasn't reached from another page.
	const back = $derived(
		feed
			? { href: routes.articles(ws, viewParam({ feed: feed.id })), label: feed.title }
			: { href: routes.articles(ws), label: 'Articles' }
	);

	async function fetchAgain() {
		fetching = true;
		try {
			const { article, failure } = await articles.resnapshot();
			if (!failure) toasts.show(`Fetched “${article.title}” again.`);
			else if (article.snapshot.state === 'captured')
				toasts.show(
					`Couldn’t fetch the page again. The article will keep its earlier text. ${failureSentence(failure)}`,
					'error'
				);
			else toasts.show(`Couldn’t fetch the page. ${failureSentence(failure)}`, 'error');
		} catch (error) {
			toasts.error(error);
		} finally {
			fetching = false;
		}
	}

	function openRename() {
		title = article?.title ?? '';
		renameError = null;
		renaming = true;
	}

	async function rename(event: SubmitEvent) {
		event.preventDefault();
		if (title.trim() === '') return;
		renameBusy = true;
		try {
			await articles.rename(title.trim());
			renaming = false;
		} catch (error) {
			const e = toProtocolError(error);
			if (e.code === 'validationFailed') renameError = e.message;
			else toasts.error(e);
		} finally {
			renameBusy = false;
		}
	}
</script>

<PageTitle title={`${article ? `${article.title} · Articles` : 'Articles'}`} />

<!-- Fill the content area (over its gutters and bottom padding) and scroll inside,
     with no scrollbar: the header's progress bar shows where the reader is. -->
<div
	class="-mb-12 h-[calc(100%+var(--s8))] [scrollbar-width:none] overflow-y-auto pb-12"
	bind:this={scroller}
	onscroll={measure}
>
	<!-- The header runs across the sheet like every page's; the title and text sit in one
	     centred column under it, at a comfortable line length. -->
	<div class="px-(--page-inset)">
		<ResourceState
			resource={articles.current}
			missing="Article not found"
			missingAction="Back to articles"
			onmissing={() => void page.goto(routes.articles(ws), { replaceState: true })}
		>
			{#snippet loaded(article)}
				<PageHeader {back} {progress}>
					{#snippet actions()}
						<Button disabled={fetching} onclick={fetchAgain}>
							{fetching ? 'Fetching…' : 'Fetch again'}
						</Button>
						<Menu
							items={[
								{
									label: 'Open original',
									onSelect: () => window.open(article.url, '_blank', 'noopener,noreferrer')
								},
								{ label: 'Rename…', onSelect: openRename },
								{
									label: article.read ? 'Mark as unread' : 'Mark as read',
									onSelect: () =>
										markRead(articles, article.id, !article.read).catch((e) => toasts.error(e))
								},
								{
									label: 'Copy address',
									onSelect: () => copyToClipboard(article.url, 'Copied the address.')
								}
							]}
						>
							{#snippet trigger(props)}
								<button
									class="inline-flex size-7.5 cursor-pointer items-center justify-center rounded-sm text-fg-muted transition-colors hover:bg-bg-hover hover:text-fg"
									type="button"
									aria-label="More actions"
									{...props}
								>
									<Ellipsis size={16} strokeWidth={1.75} />
								</button>
							{/snippet}
						</Menu>
					{/snippet}
				</PageHeader>

				<div class="mx-auto max-w-240" bind:this={content}>
					<div class="mb-6">
						<h1 class="font-serif text-xl leading-tight">{article.title}</h1>
						<p class="mt-1 text-xs text-fg-subtle">
							{#if feed}
								<a
									class="text-fg-muted hover:text-fg"
									href={routes.articles(ws, viewParam({ feed: feed.id }))}>{feed.title}</a
								> ·
							{/if}
							{#if article.author}{article.author} ·{/if}
							{publishedDate(article)}
						</p>
					</div>

					{#if article.snapshot.state === 'failed'}
						<div class="rounded-sm border border-border bg-bg-raised p-3">
							<p class="font-medium">Couldn’t fetch this page.</p>
							<p class="mt-1 text-fg-muted">
								{failureSentence(article.snapshot.failure)} Tried {ago(
									article.snapshot.attempted_at
								)}.
							</p>
						</div>
					{:else}
						{#if article.snapshot.truncated}
							<p class="mb-4 text-xs text-fg-subtle">
								The page was too long. Only part of its text is shown.
							</p>
						{/if}
						<div class="reading">
							<Markdown source={article.contents} />
						</div>
					{/if}
				</div>
			{/snippet}
		</ResourceState>
	</div>
</div>

<Dialog bind:open={renaming} title="Rename article">
	<form id="rename-article-form" onsubmit={rename}>
		<Field label="Title" for="rename-article-title" error={renameError}>
			<TextInput
				id="rename-article-title"
				bind:value={title}
				autocomplete="off"
				oninput={() => (renameError = null)}
			/>
		</Field>
	</form>
	{#snippet footer()}
		<Button onclick={() => (renaming = false)}>Cancel</Button>
		<Button
			variant="primary"
			type="submit"
			form="rename-article-form"
			disabled={renameBusy || title.trim() === ''}
		>
			Rename
		</Button>
	{/snippet}
</Dialog>

<!-- The article's text is rendered Markdown, so its elements are styled here: set
     for reading, in a serif at a larger size, its pictures shown whole. -->
<style>
	.reading :global(.markdown) {
		font-family: var(--font-serif);
		font-size: var(--text-md);
		line-height: 1.7;
	}

	.reading :global(.markdown p),
	.reading :global(.markdown ul),
	.reading :global(.markdown ol),
	.reading :global(.markdown blockquote) {
		margin-bottom: 1.1em;
	}

	.reading :global(.markdown h1),
	.reading :global(.markdown h2),
	.reading :global(.markdown h3) {
		line-height: 1.3;
	}

	.reading :global(.markdown img) {
		max-width: 100%;
		max-height: 70dvh;
		margin: 0 auto;
		opacity: 1;
	}
</style>
