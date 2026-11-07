<!--
	What the user says to an actor: a capsule holding an attach button, a
	growing field and a send button, floating over a transcript's foot or
	standing on its own for a new session, with the images to send in a tray on
	top. With `tools` (the new session's pickers), the field takes the full
	width over a row of the attach and send buttons, and the tools sit
	under the capsule on a solid tray of their own, inset from its sides. Images can also be pasted or dropped anywhere on it. Enter
	sends, Shift+Enter breaks the line.
-->
<script lang="ts">
	import type { Snippet } from 'svelte';
	import ArrowUp from '@lucide/svelte/icons/arrow-up';
	import ImagePlus from '@lucide/svelte/icons/image-plus';
	import type { ImageUpload } from '$lib/protocol/types';
	import ImageTray from '$lib/ui/ImageTray.svelte';
	import { attachImages, imageFiles } from '$lib/ui/images';

	interface Props {
		value: string;
		/** Images to send with the text, shown in the tray until sent. */
		images: ImageUpload[];
		disabled: boolean;
		placeholder: string;
		onsend: () => void;
		/** Drawn on a tray under the capsule, inset from its sides. */
		tools?: Snippet;
		/** Shown under the field, until the next send. */
		error?: string | null;
	}

	let {
		value = $bindable(),
		images = $bindable(),
		disabled,
		placeholder,
		onsend,
		tools,
		error = null
	}: Props = $props();

	const id = $props.id();
	let textarea = $state<HTMLTextAreaElement>();
	let picker = $state<HTMLInputElement>();
	let dragging = $state(false);
	const canSend = $derived(!disabled && value.trim().length > 0);

	// Grow with the text, up to the cap in CSS.
	$effect(() => {
		void value;
		if (!textarea) return;
		textarea.style.height = 'auto';
		textarea.style.height = `${textarea.scrollHeight}px`;
	});

	$effect(() => {
		textarea?.focus();
	});

	/** Attaches the images among `files`, as many as fit. */
	async function attach(files: File[]) {
		images = await attachImages(images, files);
		textarea?.focus();
	}

	function onpaste(event: ClipboardEvent) {
		const files = imageFiles(event.clipboardData?.files);
		if (files.length === 0 || disabled) return;
		event.preventDefault();
		void attach(files);
	}

	function ondragover(event: DragEvent) {
		if (disabled || !event.dataTransfer?.types.includes('Files')) return;
		event.preventDefault();
		dragging = true;
	}

	function ondrop(event: DragEvent) {
		dragging = false;
		const files = imageFiles(event.dataTransfer?.files);
		if (files.length === 0 || disabled) return;
		event.preventDefault();
		void attach(files);
	}

	function onpick() {
		void attach(imageFiles(picker?.files));
		if (picker) picker.value = '';
	}

	function onkeydown(event: KeyboardEvent) {
		if (event.key !== 'Enter' || event.isComposing || event.shiftKey) return;
		event.preventDefault();
		if (canSend) onsend();
	}

	function onsubmit(event: SubmitEvent) {
		event.preventDefault();
		if (canSend) onsend();
	}
</script>

<!-- Borderless: the capsule is the field. One line is as tall as the button. -->
{#snippet field()}
	<textarea
		class={[
			'max-h-[40vh] min-w-0 flex-1 resize-none overflow-y-auto px-2 py-1.5 text-md leading-(--leading) text-fg outline-none placeholder:text-fg-subtle focus-visible:shadow-none disabled:cursor-not-allowed',
			tools && 'min-h-28 px-3 py-2.5'
		]}
		bind:this={textarea}
		bind:value
		rows={1}
		{placeholder}
		{disabled}
		aria-label="Message"
		aria-describedby={error ? `${id}-error` : undefined}
		{onkeydown}
		{onpaste}></textarea>
{/snippet}

{#snippet attachButton()}
	<button
		class="inline-flex size-9 flex-none cursor-pointer items-center justify-center rounded-full text-fg-subtle transition-colors enabled:hover:bg-bg-hover enabled:hover:text-fg disabled:cursor-default disabled:opacity-40"
		type="button"
		{disabled}
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
{/snippet}

{#snippet send()}
	<button
		class="inline-flex size-9 flex-none cursor-pointer items-center justify-center rounded-full bg-primary text-primary-fg transition-colors enabled:hover:bg-primary-hover disabled:cursor-default disabled:opacity-40"
		type="submit"
		disabled={!canSend}
		aria-label="Send"
	>
		<ArrowUp size={18} strokeWidth={2.25} />
	</button>
{/snippet}

{#snippet capsule()}
	<form
		class={[
			'pointer-events-auto flex flex-col gap-1 rounded-lg border border-border bg-[color-mix(in_srgb,var(--bg-floating)_72%,transparent)] shadow-[var(--shadow-floating)] backdrop-blur-xl backdrop-saturate-150 transition-[border-color,box-shadow] focus-within:border-border-strong',
			tools ? 'p-2' : 'p-1.5',
			dragging && 'shadow-focus'
		]}
		{onsubmit}
		{ondragover}
		ondragleave={() => (dragging = false)}
		{ondrop}
	>
		{#if images.length > 0}
			<ImageTray bind:images />
		{/if}
		{#if tools}
			{@render field()}
			<div class="flex items-center justify-between">
				{@render attachButton()}
				{@render send()}
			</div>
		{:else}
			<div class="flex items-end gap-1">
				{@render attachButton()}
				{@render field()}
				{@render send()}
			</div>
		{/if}
		{#if error}
			<p
				class="px-2.5 pt-1 pb-1.5 text-xs text-fg-muted before:mr-2 before:inline-block before:size-1.5 before:rounded-full before:bg-danger before:align-middle"
				id="{id}-error"
				role="alert"
			>
				{error}
			</p>
		{/if}
	</form>
{/snippet}

{#if tools}
	<div>
		{@render capsule()}
		<!-- Solid, not blurred like the capsule over it, and inset past its
		     rounded corners so its sides meet the capsule's straight edge. -->
		<div
			class="mx-[calc(var(--radius-lg)+var(--s1))] flex min-w-0 items-center rounded-b-md border border-t-0 border-border bg-bg-raised py-1 pr-2 pl-4 shadow-[var(--shadow-floating)]"
		>
			{@render tools()}
		</div>
	</div>
{:else}
	{@render capsule()}
{/if}
