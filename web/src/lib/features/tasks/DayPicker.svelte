<!--
	A popover that picks a task's start or due day, with a switch between
	the two: type a phrase (the field has focus on open) ("fri", "oct 20", "in 3 days") and press Enter,
	take a quick option, or pick from the month calendar. The start stays on
	or before the due day. With `single` it picks one plain day, kept in `due`,
	without the switch or the start.
-->
<script lang="ts">
	import ChevronLeft from '@lucide/svelte/icons/chevron-left';
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import { parseDate } from '@internationalized/date';
	import { Calendar, Popover } from 'bits-ui';
	import type { Snippet } from 'svelte';
	import { dayToDate, localDay, longDay } from '$lib/format/dates';
	import Button from '$lib/ui/Button.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';
	import { returnFocusFromKeyboard } from '$lib/ui/returnFocus';
	import { parseDay } from './naturalDay';

	type Which = 'start' | 'due';

	interface Props {
		open?: boolean;
		start: string | null;
		due: string | null;
		/** The day the picker opens on. */
		initial?: Which;
		/** Picks one day, kept in `due`, instead of a task's start and due days. */
		single?: boolean;
		onchange: (start: string | null, due: string | null) => void;
		/** Where focus goes when the picker closes, instead of its trigger. */
		returnTo?: HTMLElement;
		/** Renders the trigger; spread `props` onto the element. */
		trigger: Snippet<[Record<string, unknown>]>;
	}

	let {
		open = $bindable(false),
		start,
		due,
		initial = 'due',
		single = false,
		onchange,
		returnTo,
		trigger
	}: Props = $props();

	let which = $state<Which>('due');

	const label = $derived(single ? 'Day' : which === 'start' ? 'Start day' : 'Due day');
	const value = $derived(which === 'start' ? start : due);
	const min = $derived(which === 'due' && !single ? start : null);
	const max = $derived(which === 'start' ? due : null);

	let text = $state('');
	let field = $state<HTMLInputElement>();

	const today = $derived(open ? localDay() : '');
	const parsed = $derived(parseDay(text, today));

	const QUICK = [
		{ label: 'Today', phrase: 'today' },
		{ label: 'Tomorrow', phrase: 'tomorrow' },
		{ label: 'This weekend', phrase: 'weekend' },
		{ label: 'Next week', phrase: 'next week' }
	];
	const quick = $derived(QUICK.map((q) => ({ ...q, day: parseDay(q.phrase, today)! })));

	/** Why `day` can't be this task's day, or null when it can. */
	function outOfRange(day: string): string | null {
		if (min !== null && day < min) return 'Before the start day';
		if (max !== null && day > max) return 'After the due day';
		return null;
	}

	const hint = $derived.by(() => {
		if (text.trim() === '')
			return { text: value ? longDay(value) : `No ${label.toLowerCase()}`, bad: false };
		if (parsed === null) return { text: 'Not a date.', bad: true };
		return { text: outOfRange(parsed) ?? longDay(parsed), bad: outOfRange(parsed) !== null };
	});

	function weekday(day: string): string {
		return dayToDate(day).toLocaleDateString(undefined, { weekday: 'short' });
	}

	function choose(day: string | null) {
		if (day !== null && outOfRange(day) !== null) return;
		if (day !== value) onchange(which === 'start' ? day : start, which === 'due' ? day : due);
		open = false;
	}

	function submit(event: SubmitEvent) {
		event.preventDefault();
		if (parsed !== null) choose(parsed);
	}

	const step =
		'inline-flex size-7 cursor-pointer items-center justify-center rounded-sm text-fg-muted hover:bg-bg-hover hover:text-fg';

	$effect(() => {
		if (!open) return;
		text = '';
		which = single ? 'due' : initial;
	});
</script>

<Popover.Root bind:open>
	<Popover.Trigger>
		{#snippet child({ props })}
			{@render trigger(props)}
		{/snippet}
	</Popover.Trigger>
	<Popover.Portal>
		<!-- Above dialogs (z-50), so it works inside one. -->
		<Popover.Content
			class="overlay-surface z-55 flex w-68 max-w-[calc(100vw-2*var(--gutter))] flex-col gap-2 p-2"
			align="start"
			sideOffset={4}
			data-picker
			onOpenAutoFocus={(event) => {
				// Straight to typing a day.
				event.preventDefault();
				field?.focus();
			}}
			{...returnFocusFromKeyboard(returnTo)}
		>
			{#if !single}
				<div class="flex gap-1" role="group" aria-label="Day to set">
					{#each ['start', 'due'] as const as day (day)}
						<button
							class={[
								'flex-1 cursor-pointer rounded-sm px-2 py-1 text-xs',
								which === day
									? 'bg-bg-hover font-medium text-fg'
									: 'text-fg-muted hover:bg-bg-hover hover:text-fg'
							]}
							type="button"
							aria-pressed={which === day}
							onclick={() => {
								which = day;
								text = '';
							}}
						>
							{day === 'start' ? 'Start' : 'Due'}
						</button>
					{/each}
				</div>
			{/if}
			<form onsubmit={submit}>
				<TextInput
					bind:value={text}
					bind:element={field}
					aria-label={label}
					placeholder="fri, oct 20, in 3 days…"
					autocomplete="off"
				/>
				<p class={['mt-1 px-0.5 text-xs', hint.bad ? 'text-danger' : 'text-fg-muted']}>
					{hint.text}
				</p>
			</form>
			<ul class="flex flex-col">
				{#each quick as q (q.phrase)}
					<li>
						<button
							class="flex w-full cursor-pointer items-center justify-between rounded-sm px-2 py-1 text-left text-fg enabled:hover:bg-bg-hover disabled:cursor-default disabled:text-fg-subtle"
							type="button"
							disabled={outOfRange(q.day) !== null}
							onclick={() => choose(q.day)}
						>
							{q.label}
							<span class="text-xs text-fg-subtle">{weekday(q.day)}</span>
						</button>
					</li>
				{/each}
			</ul>
			<Calendar.Root
				class="border-t border-border pt-2"
				type="single"
				value={value ? parseDate(value) : undefined}
				onValueChange={(date) => date && choose(date.toString())}
				minValue={min ? parseDate(min) : undefined}
				maxValue={max ? parseDate(max) : undefined}
				weekdayFormat="short"
				fixedWeeks
			>
				{#snippet children({ months, weekdays })}
					<Calendar.Header class="mb-1 flex items-center justify-between">
						<Calendar.PrevButton class={step} aria-label="Previous month">
							<ChevronLeft size={15} strokeWidth={1.75} />
						</Calendar.PrevButton>
						<Calendar.Heading class="text-xs font-medium" />
						<Calendar.NextButton class={step} aria-label="Next month">
							<ChevronRight size={15} strokeWidth={1.75} />
						</Calendar.NextButton>
					</Calendar.Header>
					{#each months as month (month.value.toString())}
						<Calendar.Grid class="w-full border-collapse text-xs">
							<Calendar.GridHead>
								<Calendar.GridRow class="flex">
									{#each weekdays as weekday (weekday)}
										<Calendar.HeadCell class="flex-1 font-normal text-fg-subtle">
											{weekday.slice(0, 2)}
										</Calendar.HeadCell>
									{/each}
								</Calendar.GridRow>
							</Calendar.GridHead>
							<Calendar.GridBody>
								{#each month.weeks as week (week[0].toString())}
									<Calendar.GridRow class="flex">
										{#each week as date (date.toString())}
											<Calendar.Cell class="flex-1" {date} month={month.value}>
												<Calendar.Day
													class="flex h-7 cursor-pointer items-center justify-center rounded-sm text-fg tabular-nums hover:bg-bg-hover data-disabled:cursor-default data-disabled:text-fg-subtle data-disabled:opacity-50 data-outside-month:text-fg-subtle data-selected:bg-primary data-selected:text-primary-fg data-today:font-semibold"
												/>
											</Calendar.Cell>
										{/each}
									</Calendar.GridRow>
								{/each}
							</Calendar.GridBody>
						</Calendar.Grid>
					{/each}
				{/snippet}
			</Calendar.Root>
			{#if value !== null}
				<Button class="self-start" onclick={() => choose(null)}>Clear {label.toLowerCase()}</Button>
			{/if}
		</Popover.Content>
	</Popover.Portal>
</Popover.Root>
