<script lang="ts">
	import { toProtocolError } from '$lib/protocol/errors';
	import { extensions } from '$lib/stores/extensions.svelte';
	import Button from '$lib/ui/Button.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import Editor from '$lib/ui/Editor.svelte';
	import type { ExtensionPreview } from '$lib/protocol/types';
	import { toasts } from '$lib/ui/toasts.svelte';

	interface Props {
		open?: boolean;
		/** Called with the daemon's reading of the script, to review before installing. */
		onreview: (file: { name: string | null; preview: ExtensionPreview }) => void;
	}

	let { open = $bindable(false), onreview }: Props = $props();

	let script = $state('');
	let fileName = $state<string | null>(null);
	let error = $state<string | null>(null);
	let busy = $state(false);
	let fileInput = $state<HTMLInputElement>();
	/** Whether a file is being dragged over the dialog. */
	let dragging = $state(false);

	$effect(() => {
		if (open) {
			script = '';
			fileName = null;
			error = null;
		}
	});

	function choose(event: Event) {
		const file = (event.currentTarget as HTMLInputElement).files?.[0];
		if (file) read(file);
	}

	function hasFiles(event: DragEvent): boolean {
		return event.dataTransfer?.types.includes('Files') ?? false;
	}

	// Capture phase, so the editor doesn't insert a dropped file's text at the
	// drop point; a dropped file replaces the script like a chosen one.
	function dragOver(event: DragEvent) {
		if (!hasFiles(event)) return;
		event.preventDefault();
		event.stopPropagation();
		dragging = true;
	}

	function dragLeave(event: DragEvent) {
		const target = event.currentTarget as HTMLElement;
		if (!target.contains(event.relatedTarget as Node | null)) dragging = false;
	}

	function drop(event: DragEvent) {
		if (!hasFiles(event)) return;
		event.preventDefault();
		event.stopPropagation();
		dragging = false;
		const file = event.dataTransfer?.files[0];
		if (file) read(file);
	}

	function read(file: File) {
		const reader = new FileReader();
		reader.onload = () => {
			script = String(reader.result ?? '');
			fileName = file.name;
			error = null;
		};
		reader.onerror = () => (error = `Couldn’t read ${file.name}.`);
		reader.readAsText(file);
	}

	async function review() {
		busy = true;
		error = null;
		try {
			const preview = await extensions.previewScript(script);
			open = false;
			onreview({ name: fileName, preview });
		} catch (e) {
			const failure = toProtocolError(e);
			if (failure.code === 'validationFailed') error = failure.message;
			else toasts.error(failure);
		} finally {
			busy = false;
		}
	}
</script>

<Dialog bind:open title="Install extension" description="Paste a script or drop a .js or .ts file.">
	<div
		class={[
			'-m-2 flex flex-col gap-3 rounded-md p-2 outline-2 -outline-offset-2 outline-transparent',
			dragging && 'outline-accent outline-dashed'
		]}
		role="group"
		aria-label="Drop a script file here"
		ondragentercapture={dragOver}
		ondragovercapture={dragOver}
		ondragleave={dragLeave}
		ondropcapture={drop}
	>
		<div class="flex items-center gap-2">
			<input
				bind:this={fileInput}
				class="visually-hidden"
				type="file"
				accept=".js,.ts,text/javascript,application/typescript"
				onchange={choose}
				tabindex="-1"
			/>
			<Button onclick={() => fileInput?.click()}>Choose file…</Button>
			<span class="text-xs text-fg-subtle">{fileName ?? ''}</span>
		</div>
		<div class="max-h-[40vh] overflow-y-auto rounded-sm border border-border-strong p-2">
			<Editor
				value={script}
				language="javascript"
				label="Extension script"
				minHeight="180px"
				placeholder={'function manifest() { … }'}
				onchange={(text) => (script = text)}
			/>
		</div>
		{#if error}<p class="text-xs whitespace-pre-wrap text-danger" role="alert">{error}</p>{/if}
	</div>
	{#snippet footer()}
		<Button onclick={() => (open = false)}>Cancel</Button>
		<Button variant="primary" onclick={review} disabled={busy || script.trim() === ''}>
			{busy ? 'Reading…' : 'Review…'}
		</Button>
	{/snippet}
</Dialog>
