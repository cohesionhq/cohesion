<!--
	Adds a filter: a property, one of the conditions its type allows, and the
	value or text the condition needs, typed for the property. The daemon
	applies it.
-->
<script lang="ts">
	import { untrack } from 'svelte';
	import type { DatabaseFilter, DatabaseProperty, DatabaseValue } from '$lib/protocol/types';
	import Button from '$lib/ui/Button.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import Field from '$lib/ui/Field.svelte';
	import Select from '$lib/ui/Select.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';
	import { conditionChoices, isNumeric, parseInput } from './values';

	interface Props {
		open?: boolean;
		properties: DatabaseProperty[];
		/** The property the dialog starts on; the first when null. */
		property: DatabaseProperty | null;
		onadd: (filter: DatabaseFilter) => void;
	}

	let { open = $bindable(false), properties, property, onadd }: Props = $props();

	let propertyId = $state('');
	let choiceId = $state('');
	let text = $state('');
	let error = $state<string | null>(null);

	$effect(() => {
		if (!open) return;
		untrack(() => {
			propertyId = String(property?.id ?? properties[0]?.id ?? '');
			choiceId = '';
			text = '';
			error = null;
		});
	});

	const chosen = $derived(properties.find((p) => String(p.id) === propertyId) ?? null);
	const choices = $derived(chosen ? conditionChoices(chosen.data_type) : []);
	const choice = $derived(choices.find((c) => c.id === choiceId) ?? choices[0] ?? null);

	function submit(event: SubmitEvent) {
		event.preventDefault();
		if (chosen === null || choice === null) return;
		let value: DatabaseValue | null = null;
		if (choice.takes === 'value') {
			const parsed = parseInput(chosen.data_type, text);
			if ('error' in parsed) {
				error = parsed.error;
				return;
			}
			if (parsed.value === null) {
				error = 'Enter a value.';
				return;
			}
			value = parsed.value;
		}
		onadd({ property_id: chosen.id, condition: choice.condition(value, text) });
		open = false;
	}
</script>

<Dialog bind:open title="Add filter">
	<form class="flex flex-col gap-4" id="filter-form" onsubmit={submit}>
		<Field label="Property" for="filter-property">
			<Select
				id="filter-property"
				bind:value={propertyId}
				options={properties.map((p) => ({ value: String(p.id), label: p.name }))}
				onchange={() => {
					choiceId = '';
					text = '';
					error = null;
				}}
			/>
		</Field>
		{#if chosen}
			<Field label="Condition" for="filter-condition">
				<Select
					id="filter-condition"
					value={choice?.id ?? ''}
					options={choices.map((c) => ({ value: c.id, label: c.label }))}
					onchange={(next) => {
						choiceId = next;
						error = null;
					}}
				/>
			</Field>
			{#if choice && choice.takes !== 'nothing'}
				<Field label="Value" for="filter-value" {error}>
					<TextInput
						id="filter-value"
						bind:value={text}
						type={chosen.data_type === 'date' && choice.takes === 'value' ? 'date' : 'text'}
						inputmode={chosen.data_type === 'integer'
							? 'numeric'
							: isNumeric(chosen.data_type)
								? 'decimal'
								: undefined}
						autocomplete="off"
						oninput={() => (error = null)}
					/>
				</Field>
			{/if}
		{/if}
	</form>
	{#snippet footer()}
		<Button onclick={() => (open = false)}>Cancel</Button>
		<Button
			variant="primary"
			type="submit"
			form="filter-form"
			disabled={chosen === null || (choice?.takes === 'text' && text === '')}
		>
			Add filter
		</Button>
	{/snippet}
</Dialog>
