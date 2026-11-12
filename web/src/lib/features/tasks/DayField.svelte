<!--
	A field that holds one day, picked with the task day picker: the button
	shows the day, and opens the picker to type a phrase or pick from the
	calendar. An empty day is "".
-->
<script lang="ts">
	import CalendarDays from '@lucide/svelte/icons/calendar-days';
	import { longDay } from '$lib/format/dates';
	import DayPicker from './DayPicker.svelte';

	interface Props {
		id?: string;
		value?: string;
		'aria-label'?: string;
	}

	let { id, value = $bindable(''), 'aria-label': ariaLabel }: Props = $props();
</script>

<DayPicker
	single
	start={null}
	due={value === '' ? null : value}
	onchange={(_, day) => (value = day ?? '')}
>
	{#snippet trigger(props)}
		<button
			class="flex h-7.5 w-full cursor-pointer items-center gap-2 rounded-sm border border-border bg-bg px-2 text-left text-fg transition-colors hover:border-border-strong focus:border-accent focus:shadow-focus focus:outline-none data-[state=open]:border-accent"
			type="button"
			{id}
			aria-label={ariaLabel}
			{...props}
		>
			<CalendarDays size={13} strokeWidth={1.75} class="flex-none text-fg-muted" />
			<span class={value === '' ? 'text-fg-subtle' : ''}>
				{value === '' ? 'Pick a day' : longDay(value)}
			</span>
		</button>
	{/snippet}
</DayPicker>
