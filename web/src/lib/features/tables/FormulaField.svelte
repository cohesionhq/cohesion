<!--
	A formula's source in the code editor, under a short account of what it
	is given and what it returns for its type, then why the daemon refused it
	(`error`) or why it failed on some rows the last time it ran (`failure`).
	In an expanded panel the editor takes the height that is left.
-->
<script lang="ts">
	import type { PropertyDataType } from '$lib/protocol/types';
	import Editor from '$lib/ui/Editor.svelte';
	import { returnHint } from './formulas';

	interface Props {
		source: string;
		/** The column's type, which the formula returns. */
		type: PropertyDataType;
		/** Why the source was refused. */
		error?: string | null;
		/** Why the formula failed on some rows when it last ran. */
		failure?: string | null;
		onchange: (source: string) => void;
		onsubmit?: () => void;
	}

	let { source, type, error = null, failure = null, onchange, onsubmit }: Props = $props();
</script>

<div class="flex flex-col gap-2 group-data-expanded/panel:min-h-0 group-data-expanded/panel:flex-1">
	<div class="flex flex-col gap-1 text-xs text-fg-muted">
		<p>
			A formula is a JavaScript function of its row. The row holds each column that isn’t a formula,
			by name: text and dates as text, numbers as numbers, a checkbox as true or false, and an empty
			value as null.
		</p>
		<p>{returnHint(type)}</p>
		<p>Renaming a column the formula reads breaks the formula until you edit it.</p>
	</div>
	<div
		class="flex max-h-[40vh] min-h-28 flex-col overflow-y-auto rounded-sm border border-border-strong p-2 group-data-expanded/panel:max-h-none group-data-expanded/panel:flex-1"
	>
		<Editor
			value={source}
			language="javascript"
			label="Formula"
			fill
			placeholder="(row) => …"
			{onchange}
			{onsubmit}
		/>
	</div>
	{#if error}
		<p class="text-xs whitespace-pre-wrap text-danger" role="alert">{error}</p>
	{:else if failure}
		<p class="text-xs whitespace-pre-wrap text-warn">
			The formula failed on some rows. {failure}
		</p>
	{/if}
</div>
