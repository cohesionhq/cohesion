<!-- The "?" dialog: every keyboard shortcut, grouped by where it works. -->
<script lang="ts">
	import Button from '$lib/ui/Button.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import { isMac, keyLabel } from './keys';
	import { shortcuts } from './shortcuts.svelte';

	const groups: { title: string; rows: { keys: string[]; label: string }[] }[] = [
		{
			title: 'Anywhere',
			rows: [
				{ keys: [keyLabel('mod', 'k')], label: 'Search or jump to…' },
				{ keys: [keyLabel('mod', ',')], label: 'Settings' },
				{
					keys: isMac
						? [keyLabel('mod', '['), keyLabel('mod', ']')]
						: [keyLabel('alt', 'left'), keyLabel('alt', 'right')],
					label: 'Go back / forward in the half in use'
				},
				{ keys: [`${keyLabel('alt')}-click`], label: 'Open a link in split view' },
				{ keys: [keyLabel('mod', '/'), '?'], label: 'Show these shortcuts' }
			]
		},
		{
			title: 'Search',
			rows: [{ keys: [keyLabel('alt', 'enter')], label: 'Open in split view' }]
		},
		{
			title: 'Tasks',
			rows: [
				{ keys: ['n'], label: 'New task' },
				{ keys: ['j', 'k'], label: 'Next / previous task' },
				{ keys: [keyLabel('space')], label: 'Complete or reopen' },
				{ keys: [keyLabel('enter')], label: 'Rename' },
				{ keys: ['o'], label: 'Edit the description' },
				{ keys: ['1–4', '0'], label: 'Set or clear the priority' },
				{ keys: [keyLabel('backspace')], label: 'Delete' },
				{ keys: [keyLabel('alt', 'up'), keyLabel('alt', 'down')], label: 'Move a folder up / down' }
			]
		},
		{
			title: 'Tasks › New task',
			rows: [
				{
					keys: [`${keyLabel('alt', '1')}–${keyLabel('alt', '4')}`, keyLabel('alt', '0')],
					label: 'Set or clear the priority'
				},
				{ keys: [keyLabel('alt', 'p')], label: 'Choose a priority' },
				{ keys: [keyLabel('alt', 't')], label: 'Choose tags' },
				{ keys: [keyLabel('alt', 'f')], label: 'Choose a folder' },
				{ keys: [keyLabel('alt', 's'), keyLabel('alt', 'd')], label: 'Set the start or due day' },
				{ keys: [keyLabel('alt', 'o')], label: 'Write a description' },
				{ keys: [keyLabel('mod', 'enter')], label: 'Add from the description' },
				{
					keys: [keyLabel('backspace')],
					label: 'Clear the folder, priority, tags, days and description'
				}
			]
		},
		{
			title: 'Tasks › Details',
			rows: [{ keys: [keyLabel('mod', 'enter')], label: 'Edit the description, then finish' }]
		},
		{
			title: 'Menus',
			rows: [
				{ keys: ['1–4', '0'], label: 'Priority: choose, or clear' },
				{ keys: ['1–5', '0'], label: 'Size: choose, or clear' },
				{ keys: ['o', 'd', 'c'], label: 'Status: open, done, cancelled' },
				{ keys: ['o', keyLabel('backspace')], label: 'Task: details, delete' }
			]
		},
		{
			title: 'Agents',
			rows: [
				{ keys: [keyLabel('enter')], label: 'Send' },
				{ keys: [keyLabel('shift', 'enter')], label: 'New line' },
				{ keys: [keyLabel('alt', 'up'), keyLabel('alt', 'down')], label: 'Move an agent up / down' }
			]
		},
		{
			title: 'Documents, journal and task descriptions',
			rows: [
				{ keys: ['@'], label: 'Look up an item to link' },
				{ keys: [keyLabel('enter'), keyLabel('tab')], label: 'Link the item chosen' }
			]
		},
		{
			title: 'Forward',
			rows: [
				{ keys: [keyLabel('enter')], label: 'Forward' },
				{ keys: [keyLabel('shift', 'enter')], label: 'New line' }
			]
		},
		{
			title: 'Settings › Developer',
			rows: [{ keys: [keyLabel('mod', 'enter')], label: 'Run the script or send the request' }]
		}
	];
</script>

<Dialog bind:open={shortcuts.open} title="Keyboard shortcuts">
	<div class="flex max-h-[60vh] flex-col gap-4 overflow-y-auto">
		{#each groups as group (group.title)}
			<section>
				<h3 class="mb-1 text-xs font-medium text-fg-subtle">{group.title}</h3>
				<dl>
					{#each group.rows as row (row.label)}
						<div
							class="flex items-baseline justify-between gap-3 border-border py-1 not-first:border-t"
						>
							<dt class="text-fg-muted">{row.label}</dt>
							<dd class="flex flex-none gap-1">
								{#each row.keys as key, i (i)}<kbd>{key}</kbd>{/each}
							</dd>
						</div>
					{/each}
				</dl>
			</section>
		{/each}
	</div>
	{#snippet footer()}
		<Button onclick={() => (shortcuts.open = false)}>Close</Button>
	{/snippet}
</Dialog>
