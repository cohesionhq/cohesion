<script lang="ts">
	import ArrowUp from '@lucide/svelte/icons/arrow-up';
	import ImagePlus from '@lucide/svelte/icons/image-plus';
	import Square from '@lucide/svelte/icons/square';
	import type { Snippet } from 'svelte';
	import type { AgentSkill, ImageUpload } from '$lib/protocol/types';
	import { matchingSkills } from '$lib/stores/skills.svelte';
	import ImageTray from '$lib/ui/ImageTray.svelte';
	import { attachImages, imageFiles } from '$lib/ui/images';
	import SkillChip from './SkillChip.svelte';

	interface Props {
		value: string;
		/** Images to attach, shown above the field until sent. */
		images: ImageUpload[];
		error: string | null;
		/** A standing note under the field, such as a reached spend limit; the error shows instead while there is one. */
		notice?: Snippet;
		onsend: () => void;
		/** Stops the agent's running turn; absent when none is running. */
		onstop?: () => void;
		onclearerror: () => void;
		/** Drawn inside the capsule before the field, such as who the message goes to. */
		leading?: Snippet;
		/** Bump to move focus into the textarea. */
		focusKey?: unknown;
		placeholder: string;
		/** Sits on the page, as the Dashboard's does, rather than floating over a feed: no shadow or blur. */
		resting?: boolean;
		/** The skills a `/` at the start of the field offers; none offered when absent. */
		skills?: AgentSkill[];
		/** The skill the message runs, shown as a chip above the field; the text is its note. */
		skill?: AgentSkill | null;
	}

	let {
		value = $bindable(),
		images = $bindable(),
		error,
		notice,
		onsend,
		onstop,
		onclearerror,
		leading,
		focusKey,
		placeholder,
		resting = false,
		skills,
		skill = $bindable(null)
	}: Props = $props();

	const id = $props.id();
	let textarea = $state<HTMLTextAreaElement>();
	let picker = $state<HTMLInputElement>();
	let dragging = $state(false);

	// Sending while the agent works hands the message to its turn, or queues
	// it; the feed marks it.
	const canSend = $derived(value.trim().length > 0 || images.length > 0 || skill !== null);

	// A `/` opening the field, on one line, offers the skills whose names hold
	// what follows it, until one is picked or Escape puts the list away.
	let dismissed = $state<string | null>(null);
	let highlighted = $state(0);
	const query = $derived(
		skills && skill === null && /^\/[^\n]*$/.test(value) ? value.slice(1) : null
	);
	const offered = $derived(
		query === null || value === dismissed ? [] : matchingSkills(skills ?? [], query)
	);
	const picking = $derived(query !== null && value !== dismissed);

	$effect(() => {
		void query;
		highlighted = 0;
	});

	function pickSkill(picked: AgentSkill) {
		skill = picked;
		value = '';
		textarea?.focus();
	}

	// Grow with the text, up to the cap in CSS.
	$effect(() => {
		void value;
		if (!textarea) return;
		// Empty, it is its one row, which needs no measuring (and no laying out
		// of a page still opening).
		if (!value) {
			textarea.style.height = '';
			return;
		}
		textarea.style.height = 'auto';
		textarea.style.height = `${textarea.scrollHeight}px`;
	});

	$effect(() => {
		void focusKey;
		textarea?.focus();
	});

	/** Attaches the images among `files`, as many as fit. */
	async function attach(files: File[]) {
		images = await attachImages(images, files);
		textarea?.focus();
	}

	function onpaste(event: ClipboardEvent) {
		const files = imageFiles(event.clipboardData?.files);
		if (files.length === 0) return;
		event.preventDefault();
		void attach(files);
	}

	function ondragover(event: DragEvent) {
		if (!event.dataTransfer?.types.includes('Files')) return;
		event.preventDefault();
		dragging = true;
	}

	function ondrop(event: DragEvent) {
		dragging = false;
		const files = imageFiles(event.dataTransfer?.files);
		if (files.length === 0) return;
		event.preventDefault();
		void attach(files);
	}

	function onpick() {
		void attach(imageFiles(picker?.files));
		if (picker) picker.value = '';
	}

	function onkeydown(event: KeyboardEvent) {
		if (picking) {
			if (event.key === 'Escape') {
				event.preventDefault();
				dismissed = value;
				return;
			}
			if (offered.length > 0 && (event.key === 'ArrowDown' || event.key === 'ArrowUp')) {
				event.preventDefault();
				const step = event.key === 'ArrowDown' ? 1 : -1;
				highlighted = (highlighted + step + offered.length) % offered.length;
				return;
			}
			if (
				offered.length > 0 &&
				(event.key === 'Tab' || (event.key === 'Enter' && !event.shiftKey && !event.isComposing))
			) {
				event.preventDefault();
				pickSkill(offered[highlighted]);
				return;
			}
		}
		if (event.key === 'Backspace' && skill && value === '') {
			event.preventDefault();
			skill = null;
			return;
		}
		if (event.key === 'Escape' && error) {
			event.preventDefault();
			onclearerror();
			return;
		}
		if (event.key !== 'Enter' || event.isComposing) return;
		const command = event.metaKey || event.ctrlKey;
		if (event.shiftKey && !command) return;
		event.preventDefault();
		if (canSend) onsend();
	}

	function onsubmit(event: SubmitEvent) {
		event.preventDefault();
		if (canSend) onsend();
	}

	const roundButton =
		'inline-flex size-9 flex-none cursor-pointer items-center justify-center rounded-full transition-colors';
</script>

<!-- A floating capsule over the feed's foot, translucent and blurring the
     messages that scroll under it: the images to attach on top, then one row
     of the attach button, the growing field and the send or stop button. Images can
     also be pasted or dropped anywhere on it. -->
<form
	class={[
		'pointer-events-auto relative flex flex-col rounded-lg border p-1.5 transition-[border-color,box-shadow] focus-within:border-border-strong',
		resting
			? 'border-border-strong/70 bg-bg'
			: 'border-border bg-[color-mix(in_srgb,var(--bg-floating)_72%,transparent)] shadow-[var(--shadow-floating)] backdrop-blur-xl backdrop-saturate-150',
		dragging && 'shadow-focus'
	]}
	{onsubmit}
	{ondragover}
	ondragleave={() => (dragging = false)}
	{ondrop}
>
	{#if picking}
		<!-- The skills a `/` offers, over the capsule; picking one makes it a chip. -->
		<div
			class="overlay-surface absolute inset-x-0 bottom-full mb-2 max-h-72 overflow-y-auto p-1"
			id="{id}-skills"
			role="listbox"
			aria-label="Skills"
		>
			{#each offered as option, index (option.id)}
				<button
					class={[
						'flex w-full cursor-pointer flex-col items-start rounded-sm px-2 py-1.5 text-left',
						index === highlighted ? 'bg-bg-hover' : 'hover:bg-bg-hover'
					]}
					id="{id}-skill-{option.id}"
					type="button"
					role="option"
					aria-selected={index === highlighted}
					tabindex={-1}
					onmousedown={(event) => event.preventDefault()}
					onclick={() => pickSkill(option)}
				>
					<span class="font-medium">{option.name}</span>
					{#if option.description}
						<span class="text-xs text-fg-subtle">{option.description}</span>
					{/if}
				</button>
			{:else}
				<p class="px-2 py-1.5 text-fg-muted">
					{(skills ?? []).length === 0 ? 'No skills yet.' : 'No skills match.'}
				</p>
			{/each}
		</div>
	{/if}
	{#if images.length > 0}
		<ImageTray bind:images />
	{/if}
	{#if skill}
		<div class="flex px-1 pt-0.5 pb-1">
			<SkillChip name={skill.name} prompt={skill.prompt} ondismiss={() => (skill = null)} />
		</div>
	{/if}
	<div class="flex items-end gap-1">
		<button
			class={[roundButton, 'text-fg-subtle hover:bg-bg-hover hover:text-fg']}
			type="button"
			aria-label="Attach images"
			onclick={() => picker?.click()}
		>
			<ImagePlus size={18} strokeWidth={1.75} />
		</button>
		<input
			class="hidden"
			type="file"
			accept="image/*"
			multiple
			bind:this={picker}
			onchange={onpick}
		/>
		{#if leading}<div class="flex flex-none items-center">{@render leading()}</div>{/if}
		<!-- Borderless: the capsule is the field. One line is as tall as the buttons. -->
		<textarea
			class="max-h-[40vh] min-w-0 flex-1 resize-none overflow-y-auto px-1 py-1.5 text-md leading-(--leading) text-fg outline-none placeholder:text-fg-subtle focus-visible:shadow-none"
			bind:this={textarea}
			bind:value
			rows={1}
			{placeholder}
			aria-label="Message"
			aria-describedby={error ? `${id}-error` : undefined}
			aria-controls={picking ? `${id}-skills` : undefined}
			aria-activedescendant={picking && offered[highlighted]
				? `${id}-skill-${offered[highlighted].id}`
				: undefined}
			{onkeydown}
			{onpaste}></textarea>
		<!-- While a turn runs, an empty field offers to stop it; typing turns it back into Send. -->
		{#if onstop && !canSend}
			<button
				class={[roundButton, 'bg-n3 text-fg hover:bg-n4']}
				type="button"
				aria-label="Stop"
				onclick={onstop}
			>
				<!-- Drawn at the Send arrow's stroke, so the two read as one control. -->
				<Square size={14} strokeWidth={2.25} aria-hidden="true" />
			</button>
		{:else}
			<button
				class={[
					roundButton,
					'bg-primary text-primary-fg enabled:hover:bg-primary-hover disabled:cursor-default disabled:bg-n3 disabled:text-fg-subtle'
				]}
				type="submit"
				disabled={!canSend}
				aria-label="Send"
			>
				<ArrowUp size={18} strokeWidth={2.25} />
			</button>
		{/if}
	</div>
	{#if error}
		<p
			class="px-2.5 pt-1 pb-1.5 text-xs text-fg-muted before:mr-2 before:inline-block before:size-1.5 before:rounded-full before:bg-danger before:align-middle"
			id="{id}-error"
			role="alert"
		>
			{error}
		</p>
	{:else if notice}
		<p
			class="px-2.5 pt-1 pb-1.5 text-xs text-fg-muted before:mr-2 before:inline-block before:size-1.5 before:rounded-full before:bg-warn before:align-middle"
		>
			{@render notice()}
		</p>
	{/if}
</form>
