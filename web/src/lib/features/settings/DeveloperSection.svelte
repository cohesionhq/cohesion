<script lang="ts">
	import { keyLabel } from '$lib/shell/keys';
	import { developer } from '$lib/stores/settings/developer.svelte';
	import Button from '$lib/ui/Button.svelte';
	import Editor from '$lib/ui/Editor.svelte';
	import { list } from '$lib/ui/list';
	import TextInput from '$lib/ui/TextInput.svelte';
	import Section from './Section.svelte';

	const code = 'rounded-sm border border-border p-2';
	const bar = 'mt-2 flex items-center gap-2';
	const output =
		'mt-3 max-h-[50vh] overflow-auto border-t border-border pt-3 text-xs whitespace-pre-wrap wrap-anywhere';
	// A console label spaces itself from the field after it.
	const label = 'font-medium [&+*]:mb-2';

	let showingDocs = $state(false);

	// The declarations only refetch while this section is open.
	$effect(() => () => developer.documentation.release());

	function runKey(event: KeyboardEvent) {
		if ((event.metaKey || event.ctrlKey) && event.key === 'Enter') {
			event.preventDefault();
			void developer.run();
		}
	}

	function consoleKey(event: KeyboardEvent) {
		if ((event.metaKey || event.ctrlKey) && event.key === 'Enter') {
			event.preventDefault();
			void developer.sendConsole();
		}
	}

	function showDocs() {
		showingDocs = true;
		void developer.documentation.load();
	}

	function download() {
		const blob = new Blob([developer.documentation.value], { type: 'application/typescript' });
		const url = URL.createObjectURL(blob);
		const link = document.createElement('a');
		link.href = url;
		link.download = 'cohesion.d.ts';
		link.click();
		URL.revokeObjectURL(url);
	}
</script>

<Section title="Script runner" description="Run JavaScript or TypeScript against your data.">
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div class={code} onkeydown={runKey}>
		<Editor
			value={developer.script}
			language="javascript"
			label="Script"
			minHeight="120px"
			onchange={(text) => (developer.script = text)}
		/>
	</div>
	<div class={bar}>
		<Button variant="primary" onclick={() => developer.run()} disabled={developer.running}>
			{developer.running ? 'Running…' : 'Run'}
		</Button>
		<kbd>{keyLabel('mod', 'enter')}</kbd>
	</div>
	{#if developer.runError}
		<p class={list.error} role="alert">{developer.runError.message}</p>
	{:else if developer.result !== null}
		<pre class={output} aria-label="Result">{developer.result}</pre>
	{/if}
</Section>

<Section title="API console" description="Send any protocol request and see the raw reply.">
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div class="flex flex-col gap-1" onkeydown={consoleKey}>
		<label class={label} for="console-type">type</label>
		<TextInput
			id="console-type"
			bind:value={developer.consoleType}
			class="max-w-80 font-mono text-xs"
			placeholder="listWorkspaces"
			autocomplete="off"
			spellcheck="false"
		/>
		<span class={label}>args <span class="text-xs text-fg-subtle">JSON, or blank</span></span>
		<div class={code}>
			<Editor
				value={developer.consoleArgs}
				language="javascript"
				label="Request args"
				minHeight="64px"
				placeholder={'{ "workspace_id": 1 }'}
				onchange={(text) => (developer.consoleArgs = text)}
			/>
		</div>
	</div>
	<div class={bar}>
		<Button
			onclick={() => developer.sendConsole()}
			disabled={developer.consoleBusy || developer.consoleType.trim() === ''}
		>
			{developer.consoleBusy ? 'Sending…' : 'Send'}
		</Button>
	</div>
	{#if developer.consoleError}
		<p class={list.error} role="alert">{developer.consoleError}</p>
	{:else if developer.consoleFrame !== null}
		<pre class={output} aria-label="Reply">{developer.consoleFrame}</pre>
	{/if}
</Section>

<Section
	title="Type declarations"
	description="Scripts, extensions and agents are written against this API."
>
	{#snippet actions()}
		{#if showingDocs && developer.documentation.loaded}
			<Button onclick={download}>Download</Button>
		{/if}
	{/snippet}
	{#if !showingDocs}
		<Button onclick={showDocs}>View cohesion.d.ts</Button>
	{:else if developer.documentation.error && !developer.documentation.loaded}
		<p class={list.error}>{developer.documentation.error.message}</p>
	{:else if developer.documentation.loaded}
		<div class={[code, 'max-h-[60vh] overflow-y-auto']}>
			<Editor
				value={developer.documentation.value}
				language="javascript"
				label="cohesion.d.ts"
				minHeight="120px"
				readonly
			/>
		</div>
	{/if}
</Section>
