<!-- An agent's avatar or a person's photo, whichever `who` is, on a disc of one size. -->
<script lang="ts">
	import AgentAvatar, { type Avatar } from './AgentAvatar.svelte';
	import Photo, { type Person } from './Photo.svelte';

	interface Props {
		who: Avatar | Person;
		/** `xs` beside a label, `sm` for list rows. */
		size?: 'xs' | 'sm';
	}

	let { who, size = 'xs' }: Props = $props();
</script>

{#if 'photo' in who}
	<Photo photo={who.photo} name={who.name} {size} />
{:else}
	<AgentAvatar name={who.name} color={who.color} emoji={who.emoji} {size} />
{/if}
