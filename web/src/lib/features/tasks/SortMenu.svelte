<!--
	Picks the order the Tasks page lists in, and whether rows show task IDs.
-->
<script lang="ts">
	import type { TaskSort } from '$lib/protocol/types';
	import type { TaskOrder } from '$lib/stores/tasks.svelte';
	import SortMenu, { type SortOption } from '$lib/ui/SortMenu.svelte';

	interface Props {
		order: TaskOrder;
		onchange: (order: TaskOrder) => void;
		showIds: boolean;
		onshowids: (show: boolean) => void;
	}

	let { order, onchange, showIds, onshowids }: Props = $props();

	const SORTS: SortOption<TaskSort>[] = [
		{ value: 'created', label: 'Date added', directions: ['Oldest first', 'Newest first'] },
		{ value: 'title', label: 'Title', directions: ['A to Z', 'Z to A'] },
		// Priority reads most urgent first; everything else starts ascending.
		{
			value: 'priority',
			label: 'Priority',
			directions: ['Lowest first', 'Highest first'],
			descendingFirst: true
		},
		{ value: 'size', label: 'Size', directions: ['Smallest first', 'Largest first'] },
		{ value: 'startDate', label: 'Start date', directions: ['Earliest first', 'Latest first'] },
		{ value: 'dueDate', label: 'Due date', directions: ['Earliest first', 'Latest first'] }
	];
</script>

<SortMenu
	sorts={SORTS}
	{order}
	noun="tasks"
	{onchange}
	extra={[
		{
			id: 'show-ids',
			label: 'Show task IDs',
			checked: showIds,
			onSelect: () => onshowids(!showIds)
		}
	]}
/>
