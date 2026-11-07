<script lang="ts">
	import ArrowUp from '@lucide/svelte/icons/arrow-up';
	import ImagePlus from '@lucide/svelte/icons/image-plus';
	import Square from '@lucide/svelte/icons/square';
	import type { Snippet } from 'svelte';
	import type { ImageUpload } from '$lib/protocol/types';
	import ImageTray from '$lib/ui/ImageTray.svelte';
	import { attachImages, imageFiles } from '$lib/ui/images';

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
		/** Bump to move focus into the textarea. */
		focusKey?: unknown;
		placeholder: string;
	}

	let {
		value = $bindable(),
		images = $bindable(),
		error,
		notice,
		onsend,
		onstop,
		onclearerror,
		focusKey,
		placeholder
	}: Props = $props();

	const id = $props.id();
	let textarea = $state<HTMLTextAreaElement>();
	let picker = $state<HTMLInputElement>();
	let dragging = $state(false);

	// Sending while the agent works hands the message to its turn, or queues
	// it; the feed marks it.
	const canSend = $derived(value.trim().length > 0 || images.length > 0);

	// Grow with the text, up to the cap in CSS.
	$effect(() => {
		void value;
		if (!textarea) return;
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
		'pointer-events-auto flex flex-col rounded-lg border border-border bg-[color-mix(in_srgb,var(--bg-floating)_72%,transparent)] p-1.5 shadow-[var(--shadow-floating)] backdrop-blur-xl backdrop-saturate-150 transition-[border-color,box-shadow] focus-within:border-border-strong',
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
		<!-- Borderless: the capsule is the field. One line is as tall as the buttons. -->
		<textarea
			class="max-h-[40vh] min-w-0 flex-1 resize-none overflow-y-auto px-1 py-1.5 text-md leading-(--leading) text-fg outline-none placeholder:text-fg-subtle focus-visible:shadow-none"
			bind:this={textarea}
			bind:value
			rows={1}
			{placeholder}
			aria-label="Message"
			aria-describedby={error ? `${id}-error` : undefined}
			{onkeydown}
			{onpaste}></textarea>
		<!-- While a turn runs, an empty field offers to stop it; typing turns it back into Send. -->
		{#if onstop && !canSend}
			<button
				class={[
					roundButton,
					'border border-border text-danger hover:border-danger/40 hover:bg-danger/10'
				]}
				type="button"
				aria-label="Stop"
				onclick={onstop}
			>
				<Square size={12} fill="currentColor" strokeWidth={0} aria-hidden="true" />
			</button>
		{:else}
			<button
				class={[
					roundButton,
					'bg-primary text-primary-fg enabled:hover:bg-primary-hover disabled:cursor-default disabled:opacity-40'
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
