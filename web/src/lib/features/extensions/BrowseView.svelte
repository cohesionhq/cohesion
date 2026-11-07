<!--
	The Browse tab: a search over every registry the user has allowed (an
	extension granted `registry`), then one section per registry, leaving out
	what's installed (its updates are offered under Installed). Install opens
	the review of the real script.
-->
<script lang="ts">
	import { untrack } from 'svelte';
	import Section from '$lib/features/settings/Section.svelte';
	import { toProtocolError } from '$lib/protocol/errors';
	import type { RegistryResult, RegistrySection } from '$lib/protocol/types';
	import { extensions } from '$lib/stores/extensions.svelte';
	import Button from '$lib/ui/Button.svelte';
	import EmptyState from '$lib/ui/EmptyState.svelte';
	import IconTile from '$lib/ui/IconTile.svelte';
	import { list } from '$lib/ui/list';
	import TextInput from '$lib/ui/TextInput.svelte';
	import ReviewExtensionDialog from './ReviewExtensionDialog.svelte';

	let query = $state('');
	/** Null until the first search answers. */
	let sections = $state<RegistrySection[] | null>(null);
	let error = $state<string | null>(null);
	let generation = 0;
	let reviewing = $state<RegistryResult | null>(null);
	let reviewOpen = $state(false);

	$effect(() => {
		untrack(() => void extensions.load());
		return () => extensions.list.release();
	});

	// Debounced; a newer query drops older answers. The first search runs at once.
	$effect(() => {
		const q = query.trim();
		const delay = untrack(() => (sections === null && error === null ? 0 : 250));
		const timer = setTimeout(() => void search(q), delay);
		return () => clearTimeout(timer);
	});

	async function search(q: string) {
		const current = ++generation;
		try {
			const found = await extensions.search(q);
			if (current !== generation) return;
			sections = found;
			error = null;
		} catch (e) {
			if (current === generation) error = toProtocolError(e).message;
		}
	}

	function notInstalled(result: RegistryResult): boolean {
		return !extensions.list.value.some((e) => e.script_identifier === result.manifest.identifier);
	}

	function review(result: RegistryResult) {
		reviewing = result;
		reviewOpen = true;
	}
</script>

{#if error !== null && sections === null}
	<EmptyState message={error} actionLabel="Try again" onaction={() => void search(query.trim())} />
{:else if sections !== null && sections.length === 0}
	<EmptyState
		message="No registries yet"
		detail="A registry is an extension that lists others. Install one from a file, then turn on “Offer extensions to browse” on its page."
	/>
{:else}
	<div class="flex flex-col gap-6">
		<TextInput
			type="search"
			placeholder="Search extensions"
			aria-label="Search extensions"
			bind:value={query}
		/>

		{#if sections === null}
			<p class="text-fg-subtle" role="status">Searching…</p>
		{:else}
			{#if error !== null}<p class="text-xs text-danger" role="alert">{error}</p>{/if}
			<div>
				{#each sections as section (section.extension_id)}
					<Section title={section.name}>
						{#if section.error !== null}
							<div class={list.empty}>
								<p>Couldn’t reach this registry.</p>
								<p class="font-mono text-2xs wrap-anywhere whitespace-pre-wrap text-fg-subtle">
									{section.error}
								</p>
							</div>
						{:else}
							<ul class={list.rows}>
								{#each section.results.filter(notInstalled) as result (result.source_url)}
									<li class="flex items-center gap-3 border-b border-border py-3">
										<IconTile icon={result.manifest.icon} title={result.manifest.name} size={32} />
										<div class="flex min-w-0 flex-1 flex-col">
											<span class="flex items-baseline gap-2">
												<span class="truncate">{result.manifest.name}</span>
												<span class="flex-none text-xs text-fg-subtle tabular-nums"
													>{result.manifest.version}</span
												>
											</span>
											<span class="text-xs text-fg-muted">
												{[
													result.manifest.author && `By ${result.manifest.author}`,
													result.manifest.description
												]
													.filter(Boolean)
													.join(' · ')}
											</span>
										</div>
										<Button
											class="border-border-strong!"
											aria-label="Install {result.manifest.name}"
											onclick={() => review(result)}>Install…</Button
										>
									</li>
								{:else}
									<li class={list.empty}>
										{query.trim() === '' ? 'Everything it lists is installed.' : 'No matches.'}
									</li>
								{/each}
							</ul>
						{/if}
					</Section>
				{/each}
			</div>
		{/if}
	</div>
{/if}

<ReviewExtensionDialog bind:open={reviewOpen} result={reviewing} />
