<!--
	A task's priority as a disc: the priority's own colour as a wash, the
	flag's outline inside it in the colour itself, so it reads apart from a
	size (a ruler, a word) and keeps its contrast in both schemes, the tokens
	having one for each. It is as tall as a size chip or a tag, so a row's line
	holds them at one weight. Named by default, "Priority: Medium", for a
	screen reader (the app has no tooltips); a control that names itself
	passes `named={false}`.
-->
<script lang="ts">
	import Flag from '@lucide/svelte/icons/flag';
	import type { TaskPriority } from '$lib/protocol/types';
	import { priorityOf } from './priority';

	interface Props {
		priority: TaskPriority;
		named?: boolean;
	}

	let { priority, named = true }: Props = $props();

	const current = $derived(priorityOf(priority));
</script>

<span
	class={[
		'inline-flex size-4.5 flex-none items-center justify-center rounded-full bg-current/20',
		current.tone
	]}
>
	<Flag size={11} strokeWidth={2} aria-hidden="true" />
	{#if named}<span class="sr-only">Priority: {current.label}</span>{/if}
</span>
