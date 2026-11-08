<!--
	The Dashboard's tasks due, under "Due": the workspace's open tasks due today or
	earlier, soonest first. Each leads with the task's own checkbox, which
	completes it here, and opens on the Tasks page, with "Overdue" in red or
	"Due today" at full strength at its end, as a task's row says them. Drawn as the recap's sections are; nothing shows
	when nothing is due.
-->
<script lang="ts">
	import { onMount } from 'svelte';
	import { localDay } from '$lib/format/dates';
	import { routes } from '$lib/shell/routes';
	import { pageStores } from '$lib/stores/pageStores.svelte';
	import Checkbox from '$lib/ui/Checkbox.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';
	import { list } from '$lib/ui/list';
	import { sectionRow } from './section';

	let { ws }: { ws: number } = $props();

	const { tasks } = pageStores();
	const today = localDay();

	onMount(() => {
		void tasks.due.load(`${ws}:${localDay(-1)}`);
		return () => tasks.due.release();
	});

	const due = $derived(tasks.due.value.tasks);
</script>

{#if due.length > 0}
	<section aria-labelledby="due-heading">
		<h2 id="due-heading" class={[list.heading, 'mb-0']}>
			Due<span class={list.count}>{due.length}</span>
		</h2>
		<ul class="-mx-2 flex flex-col pt-1">
			{#each due as task (task.id)}
				{@const overdue = task.due_date !== null && task.due_date < today}
				<li class="relative">
					<!-- The checkbox sits over the row's 20px leading slot, outside the link. -->
					<span class="absolute top-0 left-2 z-1 flex h-9 w-5 items-center justify-center">
						<Checkbox
							round
							checked={task.status !== 'incomplete'}
							aria-label="Complete “{task.title}”"
							onchange={() => tasks.toggle(task).catch((e) => toasts.error(e))}
						/>
					</span>
					<a class={sectionRow} href={routes.task(ws, task.id)}>
						<span class="w-5 flex-none"></span>
						<span class="min-w-0 flex-1 truncate">{task.title}</span>
						<span class={['flex-none text-xs', overdue ? 'text-danger' : 'text-fg']}
							>{overdue ? 'Overdue' : 'Due today'}</span
						>
					</a>
				</li>
			{/each}
		</ul>
	</section>
{/if}
