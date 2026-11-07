<!--
	Picks the order a page lists in: a key, then a direction named for that
	key ("Newest first", "Z to A").
-->
<script lang="ts" module>
	/** A key with its label and its ascending and descending directions. */
	export interface SortOption<S extends string> {
		value: S;
		label: string;
		directions: [string, string];
		/** Picking it starts descending, as most urgent or newest first reads. */
		descendingFirst?: boolean;
	}

	export interface SortOrder<S extends string> {
		sort: S;
		descending: boolean;
	}
</script>

<script lang="ts" generics="S extends string">
	import { secondaryClass } from './Button.svelte';
	import ArrowUpDown from '@lucide/svelte/icons/arrow-up-down';
	import Menu, { type MenuItem } from '$lib/ui/Menu.svelte';

	interface Props {
		sorts: SortOption<S>[];
		order: SortOrder<S>;
		/** What's listed, for the trigger's aria-label: "tasks". */
		noun: string;
		onchange: (order: SortOrder<S>) => void;
		/** Items appended after the directions, set off by a rule. */
		extra?: MenuItem[];
	}

	let { sorts, order, noun, onchange, extra = [] }: Props = $props();

	const current = $derived(sorts.find((s) => s.value === order.sort) ?? sorts[0]);
	const items = $derived<MenuItem[]>([
		...sorts.map((s) => ({
			id: s.value,
			label: s.label,
			checked: s.value === order.sort,
			onSelect: () => onchange({ sort: s.value, descending: s.descendingFirst === true })
		})),
		...current.directions.map((label, i) => ({
			id: `direction-${i}`,
			label,
			checked: order.descending === (i === 1),
			separated: i === 0,
			onSelect: () => onchange({ sort: order.sort, descending: i === 1 })
		})),
		...extra.map((item, i) => ({ ...item, separated: i === 0 }))
	]);
</script>

<Menu {items}>
	{#snippet trigger(props)}
		<button
			class={[
				'inline-flex h-7.5 cursor-pointer items-center gap-1.5 rounded-sm border px-2.5 font-medium transition-colors',
				secondaryClass
			]}
			type="button"
			aria-label="Sort {noun}: {current.label}, {current.directions[order.descending ? 1 : 0]}"
			{...props}
		>
			<ArrowUpDown size={14} strokeWidth={1.75} />
			{current.label}
		</button>
	{/snippet}
</Menu>
