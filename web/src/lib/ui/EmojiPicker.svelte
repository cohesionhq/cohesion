<!--
	Picks the optional emoji shown in place of a workspace's or agent's
	initial: a square button showing the choice, which opens a grid of emoji
	by category with a search box. A pasted emoji the set lacks can still be
	used from the search box. The panel sits above dialogs, where it is used.
-->
<script lang="ts">
	import SmilePlus from '@lucide/svelte/icons/smile-plus';
	import Popover from './Popover.svelte';
	import TextInput from './TextInput.svelte';
	import { EMOJI_GROUPS, hasEmoji, searchEmoji, type Emoji } from './emoji';

	interface Props {
		value?: string;
	}

	let { value = $bindable('') }: Props = $props();

	const COLUMNS = 8;
	let open = $state(false);
	let query = $state('');
	let search = $state<HTMLInputElement>();
	let scroller = $state<HTMLElement>();
	let activeGroup = $state(EMOJI_GROUPS[0].name);

	const results = $derived(searchEmoji(query));
	// A typed or pasted emoji is offered as-is when the set has no match for it.
	const pasted = $derived(results.length === 0 && hasEmoji(query) ? query.trim() : null);

	// Each open starts on the full grid, scrolled to the top.
	$effect(() => {
		if (!open) return;
		query = '';
		activeGroup = EMOJI_GROUPS[0].name;
	});

	function choose(char: string) {
		value = char;
		open = false;
	}

	function emojiButtons() {
		return [...(scroller?.querySelectorAll<HTMLButtonElement>('button[data-emoji]') ?? [])];
	}

	function onSearchKey(event: KeyboardEvent) {
		if (event.key === 'Enter') {
			event.preventDefault();
			const first = pasted ?? results[0]?.char;
			if (first) choose(first);
		} else if (event.key === 'ArrowDown') {
			event.preventDefault();
			emojiButtons()[0]?.focus();
		}
	}

	// Arrow keys move through the grid; up from the top row returns to the search box.
	function onGridKey(event: KeyboardEvent) {
		const step = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -COLUMNS, ArrowDown: COLUMNS }[event.key];
		if (step === undefined) return;
		const buttons = emojiButtons();
		const index = buttons.indexOf(event.target as HTMLButtonElement);
		if (index === -1) return;
		event.preventDefault();
		const next = index + step;
		if (next < 0) search?.focus();
		else buttons[Math.min(next, buttons.length - 1)].focus();
	}

	function showGroup(name: string) {
		scroller?.querySelector(`[data-group="${name}"]`)?.scrollIntoView({ block: 'start' });
		activeGroup = name;
	}

	// The highlighted tab follows the group at the top of the scrolled grid.
	function onScroll() {
		if (!scroller) return;
		const top = scroller.scrollTop + 8;
		for (const section of scroller.querySelectorAll<HTMLElement>('[data-group]')) {
			if (section.offsetTop <= top) activeGroup = section.dataset.group!;
		}
	}

	const tab =
		'inline-grid size-7.5 cursor-pointer place-items-center rounded-sm text-md leading-none hover:bg-bg-hover hover:opacity-100 focus-visible:shadow-focus focus-visible:outline-none';
	const cell =
		'inline-grid size-8 cursor-pointer place-items-center rounded-sm text-lg leading-none hover:bg-bg-hover focus-visible:bg-bg-hover focus-visible:shadow-focus focus-visible:outline-none';
</script>

{#snippet emojiCell(e: Emoji)}
	<button
		class={cell}
		type="button"
		data-emoji
		aria-label={e.words.split(' ')[0]}
		onclick={() => choose(e.char)}>{e.char}</button
	>
{/snippet}

<Popover bind:open inDialog>
	{#snippet trigger(props)}
		<button
			class="inline-grid size-8 flex-none cursor-pointer place-items-center rounded-sm border border-border-strong bg-bg text-md leading-none text-fg-subtle hover:bg-bg-hover focus-visible:border-accent focus-visible:shadow-focus focus-visible:outline-none data-[state=open]:border-accent"
			type="button"
			{...props}
			aria-label={value ? `Emoji: ${value}` : 'Choose an emoji'}
		>
			{#if value}
				<span class="text-fg">{value}</span>
			{:else}
				<SmilePlus size={16} strokeWidth={1.75} />
			{/if}
		</button>
	{/snippet}
	<div class="flex w-68 flex-col gap-1">
		<TextInput
			bind:element={search}
			bind:value={query}
			onkeydown={onSearchKey}
			placeholder="Search emoji"
			aria-label="Search emoji"
			autocomplete="off"
		/>
		{#if query.trim() === ''}
			<div class="flex justify-between" role="toolbar" aria-label="Emoji categories">
				{#each EMOJI_GROUPS as g (g.name)}
					<button
						class={[tab, g.name === activeGroup ? 'bg-bg-hover' : 'opacity-70']}
						type="button"
						aria-label={g.name}
						aria-pressed={g.name === activeGroup}
						onclick={() => showGroup(g.name)}>{g.icon}</button
					>
				{/each}
			</div>
		{/if}
		<!-- Arrow keys from the emoji buttons inside bubble up to move between them. -->
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div
			class="relative h-64 overflow-y-auto border-t border-border pt-1"
			bind:this={scroller}
			onscroll={onScroll}
			onkeydown={onGridKey}
		>
			{#if query.trim() === ''}
				{#each EMOJI_GROUPS as g (g.name)}
					<section data-group={g.name}>
						<h3 class="px-1 pt-1 pb-0.5 text-2xs font-medium text-fg-subtle">{g.name}</h3>
						<div class="grid grid-cols-8">
							{#each g.emoji as e (e.char)}{@render emojiCell(e)}{/each}
						</div>
					</section>
				{/each}
			{:else if results.length > 0}
				<div class="grid grid-cols-8">
					{#each results as e (e.char)}{@render emojiCell(e)}{/each}
				</div>
			{:else if pasted}
				<button
					class="flex w-full cursor-pointer items-center gap-2 rounded-sm px-2 py-1 text-left text-fg hover:bg-bg-hover focus-visible:bg-bg-hover focus-visible:outline-none"
					type="button"
					data-emoji
					onclick={() => choose(pasted)}
				>
					<span class="text-lg leading-none">{pasted}</span> Use this emoji
				</button>
			{:else}
				<p class="px-2 py-3 text-xs text-fg-subtle">No emoji match “{query.trim()}”.</p>
			{/if}
		</div>
		{#if value}
			<div class="border-t border-border pt-1">
				<button
					class="w-full cursor-pointer rounded-sm px-2 py-1 text-left text-fg-muted hover:bg-bg-hover hover:text-fg"
					type="button"
					onclick={() => choose('')}
				>
					Remove emoji
				</button>
			</div>
		{/if}
	</div>
</Popover>
