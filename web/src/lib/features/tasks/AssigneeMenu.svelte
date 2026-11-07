<!--
	Who a task is assigned to, as a button that picks who: their avatar, or a
	muted profile icon while no one is.
-->
<script lang="ts">
	import CircleUser from '@lucide/svelte/icons/circle-user';
	import Avatar from '$lib/ui/Avatar.svelte';
	import Menu, { type MenuItem } from '$lib/ui/Menu.svelte';
	import type { ShownAssignee } from './assignee';

	interface Props {
		assignee: ShownAssignee | null;
		/** Names the task in the trigger's label. */
		title: string;
		items: MenuItem[];
	}

	let { assignee, title, items }: Props = $props();
</script>

<Menu {items}>
	{#snippet trigger(props)}
		<button
			class="inline-flex size-7 flex-none cursor-pointer items-center justify-center rounded-sm text-fg-subtle hover:bg-bg-hover hover:text-fg data-[state=open]:bg-bg-hover data-[state=open]:text-fg"
			type="button"
			aria-label={assignee
				? `Assign to: ${title}, assigned to ${assignee.label}`
				: `Assign to: ${title}`}
			{...props}
		>
			{#if assignee}
				<Avatar who={assignee.avatar} />
			{:else}
				<CircleUser size={15} strokeWidth={1.75} />
			{/if}
		</button>
	{/snippet}
</Menu>
