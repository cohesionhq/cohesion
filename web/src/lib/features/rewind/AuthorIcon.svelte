<!-- Who made a version, as a small symbol beside their name. -->
<script lang="ts">
	import Bot from '@lucide/svelte/icons/bot';
	import CodeXml from '@lucide/svelte/icons/code-xml';
	import Puzzle from '@lucide/svelte/icons/puzzle';
	import User from '@lucide/svelte/icons/user';
	import type { VersionAuthor } from '$lib/protocol/types';

	let { author }: { author: VersionAuthor } = $props();

	const icons = { user: User, agent: Bot, extension: Puzzle, script: CodeXml } as const;
	const Icon = $derived(author === 'unknown' ? null : icons[author]);
</script>

{#if Icon}
	<Icon size={13} strokeWidth={1.75} aria-hidden="true" class="flex-none" />
{/if}
