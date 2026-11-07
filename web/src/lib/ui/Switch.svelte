<!-- An on/off control for a setting that applies at once, such as a grant. -->
<script lang="ts">
	import { Switch } from 'bits-ui';

	interface Props {
		checked: boolean;
		/** Called with the new state; the switch shows `checked`, so the caller decides. */
		onchange: (checked: boolean) => void;
		disabled?: boolean;
		/** The control's accessible name, when no visible label points at it. */
		label?: string;
		id?: string;
	}

	let { checked, onchange, disabled = false, label, id }: Props = $props();
</script>

<Switch.Root
	class="inline-flex h-5 w-8.5 flex-none cursor-pointer items-center rounded-full border border-transparent bg-n4 p-0.5 transition-colors disabled:cursor-default disabled:opacity-50 data-[state=checked]:bg-accent"
	bind:checked={() => checked, onchange}
	{disabled}
	{id}
	aria-label={label}
>
	<Switch.Thumb
		class="pointer-events-none block size-4 rounded-full bg-(--knob) shadow-knob transition-transform data-[state=checked]:translate-x-3.5"
	/>
</Switch.Root>
