<!--
	Pages through a list the daemon hands out a page at a time: the range
	shown, the way to the page before and after, and how many a page holds.
	Hidden while the whole list fits the smallest page.
-->
<script lang="ts" module>
	export const PAGE_SIZES = [25, 50, 100];
	const DEFAULT_SIZE = 50;
	const SIZE_KEY = 'cohesion.pageSize';

	/** The page size this viewer last picked; 50 at first. */
	export function rememberedPageSize(): number {
		try {
			const stored = Number(localStorage.getItem(SIZE_KEY));
			return PAGE_SIZES.includes(stored) ? stored : DEFAULT_SIZE;
		} catch {
			return DEFAULT_SIZE;
		}
	}

	function rememberPageSize(size: number): void {
		try {
			localStorage.setItem(SIZE_KEY, String(size));
		} catch {
			// Remembering the size is a convenience; ignore blocked storage.
		}
	}
</script>

<script lang="ts">
	import ChevronLeft from '@lucide/svelte/icons/chevron-left';
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import Select from './Select.svelte';

	interface Props {
		/** How many rows come before the page shown. */
		offset: number;
		/** How many rows a page holds. */
		limit: number;
		/** How many rows the whole list has. */
		total: number;
		/** Shows the page starting at `offset`. */
		onpage: (offset: number) => void;
		/** Shows pages of `limit` rows, from the first. */
		onsize: (limit: number) => void;
	}

	let { offset, limit, total, onpage, onsize }: Props = $props();

	const sizeOptions = PAGE_SIZES.map((n) => ({ value: String(n), label: `${n} per page` }));
	const tool =
		'inline-flex size-7.5 cursor-pointer items-center justify-center rounded-sm text-fg-muted transition-colors hover:bg-bg-hover hover:text-fg disabled:cursor-default disabled:opacity-40 disabled:hover:bg-transparent';

	function pickSize(value: string) {
		const size = Number(value);
		rememberPageSize(size);
		onsize(size);
	}
</script>

{#if total > PAGE_SIZES[0]}
	<nav class="mt-4 flex items-center gap-2 text-xs text-fg-muted" aria-label="Pages">
		<span class="tabular-nums">{offset + 1}–{Math.min(offset + limit, total)} of {total}</span>
		<button
			class={tool}
			type="button"
			aria-label="Previous page"
			disabled={offset === 0}
			onclick={() => onpage(Math.max(0, offset - limit))}
		>
			<ChevronLeft size={16} strokeWidth={1.75} />
		</button>
		<button
			class={tool}
			type="button"
			aria-label="Next page"
			disabled={offset + limit >= total}
			onclick={() => onpage(offset + limit)}
		>
			<ChevronRight size={16} strokeWidth={1.75} />
		</button>
		<span class="ml-auto">
			<Select
				value={String(limit)}
				options={sizeOptions}
				aria-label="Page size"
				onchange={pickSize}
			/>
		</span>
	</nav>
{/if}
