<!--
	A flag that picks a task's priority, coloured by it, showing while the row
	is hovered or focused (the row's line names a set priority).
-->
<script lang="ts">
	import Flag from '@lucide/svelte/icons/flag';
	import type { TaskPriority } from '$lib/protocol/types';
	import Menu, { type MenuItem } from '$lib/ui/Menu.svelte';
	import { priorities, priorityOf } from './priority';

	interface Props {
		priority: TaskPriority;
		/** Names the task in the trigger's label. */
		title: string;
		onchange: (priority: TaskPriority) => void;
	}

	let { priority, title, onchange }: Props = $props();

	const current = $derived(priorityOf(priority));
	const items = $derived<MenuItem[]>(
		priorities().map((p) => ({
			id: p.value,
			label: p.label,
			key: p.key,
			checked: p.value === priority,
			onSelect: () => onchange(p.value)
		}))
	);
</script>

<Menu {items}>
	{#snippet trigger(props)}
		<button
			class={[
				'inline-flex size-7 cursor-pointer items-center justify-center rounded-sm hover:bg-bg-hover data-[state=open]:bg-bg-hover',
				current.tone,
				// It shows with the row's other actions; a set priority reads in the row's line.
				'opacity-0 transition-opacity group-focus-within/row:opacity-100 group-hover/row:opacity-100 data-[state=open]:opacity-100 touch:opacity-100'
			]}
			type="button"
			aria-label="Priority of {title}: {current.label}"
			{...props}
		>
			<Flag size={15} strokeWidth={1.75} />
		</button>
	{/snippet}
</Menu>
