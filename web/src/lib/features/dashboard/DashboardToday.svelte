<!--
	The Dashboard's tasks due, under "Due": the workspace's open tasks due today or
	earlier, soonest first, each opening on the Tasks page, with "Overdue" in
	red or "Due today" at its end. Drawn as the recap's sections are; nothing
	shows when nothing is due.
-->
<script lang="ts">
	import CalendarClock from '@lucide/svelte/icons/calendar-clock';
	import { onMount } from 'svelte';
	import { localDay } from '$lib/format/dates';
	import { routes } from '$lib/shell/routes';
	import { pageStores } from '$lib/stores/pageStores.svelte';

	let { ws }: { ws: number } = $props();

	const { tasks } = pageStores();
	const today = localDay();

	onMount(() => {
		void tasks.due.load(`${ws}:${localDay(-1)}`);
		return () => tasks.due.release();
	});

	const due = $derived(tasks.due.value.tasks);

	const row =
		'flex h-9 items-center gap-2.5 rounded-md px-2 text-fg no-underline transition-colors hover:bg-bg-hover hover:no-underline';
</script>

{#if due.length > 0}
	<section aria-labelledby="due-heading">
		<h2 id="due-heading" class="border-b border-border pb-2 text-xs font-semibold text-fg-muted">
			Due
		</h2>
		<ul class="-mx-2 flex flex-col pt-1">
			{#each due as task (task.id)}
				{@const overdue = task.due_date !== null && task.due_date < today}
				<li>
					<a class={row} href={routes.task(ws, task.id)}>
						<!-- The avatar's 20px slot, as the recap's rows draw their icons. -->
						<span class="flex w-5 flex-none justify-center text-fg-subtle">
							<CalendarClock size={16} strokeWidth={1.75} aria-hidden="true" />
						</span>
						<span class="min-w-0 flex-1 truncate">{task.title}</span>
						<span class={['flex-none text-xs', overdue ? 'text-danger' : 'text-fg-subtle']}
							>{overdue ? 'Overdue' : 'Due today'}</span
						>
					</a>
				</li>
			{/each}
		</ul>
	</section>
{/if}
