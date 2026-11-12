<!--
	The Dashboard's pinned documents, under "Pinned": the workspace's documents pinned
	from their own pages, most recently changed first, as the user's working set. Each opens its document, and
	unpins from its end. Drawn as the other sections are; nothing shows when nothing is pinned.
-->
<script lang="ts">
	import FileText from '@lucide/svelte/icons/file-text';
	import PinOff from '@lucide/svelte/icons/pin-off';
	import { onMount } from 'svelte';
	import { routes } from '$lib/shell/routes';
	import { pageStores } from '$lib/stores/pageStores.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';
	import { list } from '$lib/ui/list';
	import { sectionRow } from './section';

	let { ws }: { ws: number } = $props();

	const { documents } = pageStores();

	onMount(() => {
		void documents.list.load(ws);
		return () => documents.list.release();
	});

	const pinned = $derived(documents.list.value.filter((d) => d.pinned));
</script>

{#if pinned.length > 0}
	<section aria-labelledby="pinned-heading">
		<h2 id="pinned-heading" class={[list.heading, 'mb-0']}>
			Pinned<span class={list.count}>{pinned.length}</span>
		</h2>
		<ul class="-mx-2 flex flex-col pt-1">
			{#each pinned as doc (doc.id)}
				<li class="group relative">
					<a class={sectionRow} href={routes.document(ws, doc.id)}>
						<FileText
							size={14}
							strokeWidth={1.75}
							class="w-5 flex-none text-fg-subtle"
							aria-hidden="true"
						/>
						<span class="min-w-0 flex-1 truncate">{doc.title}</span>
						<span class="w-7 flex-none"></span>
					</a>
					<button
						class="absolute top-1 right-1.5 inline-flex size-7 cursor-pointer items-center justify-center rounded-sm text-fg-muted opacity-0 transition-colors group-focus-within:opacity-100 group-hover:opacity-100 hover:bg-bg-hover hover:text-fg"
						type="button"
						aria-label="Unpin “{doc.title}”"
						onclick={() => documents.setPinned(ws, doc.id, false).catch((e) => toasts.error(e))}
					>
						<PinOff size={13} strokeWidth={1.75} aria-hidden="true" />
					</button>
				</li>
			{/each}
		</ul>
	</section>
{/if}
