<!--
	A workspace's contacts, by last name, each row its photo, its name and the
	first way to reach them; a row opens the contact's panel, where it is edited.
	A link or search lands here with `?contact=`, which opens that contact's panel.
-->
<script lang="ts">
	import Plus from '@lucide/svelte/icons/plus';
	import { untrack } from 'svelte';
	import ContactDialog from '$lib/features/contacts/ContactDialog.svelte';
	import ContactPanel from '$lib/features/contacts/ContactPanel.svelte';
	import { workspaceLoadError } from '$lib/shell/loadError';
	import { pane } from '$lib/shell/pane.svelte';
	import PageTitle from '$lib/shell/PageTitle.svelte';
	import { fullName } from '$lib/stores/contacts.svelte';
	import { pageStores } from '$lib/stores/pageStores.svelte';
	import Button from '$lib/ui/Button.svelte';
	import EmptyState from '$lib/ui/EmptyState.svelte';
	import PageHeader from '$lib/ui/PageHeader.svelte';
	import Photo from '$lib/ui/Photo.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';

	const { contacts } = pageStores();

	const page = pane();

	const ws = $derived(Number(page.params.ws));
	let query = $state('');
	let creating = $state(false);
	let openId = $state<number | null>(null);
	let panelOpen = $state(false);

	$effect(() => {
		const id = ws;
		untrack(() => void contacts.load(id));
		query = '';
		panelOpen = false;
		return () => contacts.list.release();
	});

	const shown = $derived.by(() => {
		const words = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
		if (words.length === 0) return contacts.list.value;
		return contacts.list.value.filter((c) => {
			const name = fullName(c).toLowerCase();
			return words.every((w) => name.includes(w));
		});
	});

	const opened = $derived(contacts.list.value.find((c) => c.id === openId) ?? null);

	function show(id: number) {
		openId = id;
		panelOpen = true;
	}

	// Open the contact `?contact=` names, then drop the parameter.
	$effect(() => {
		const param = page.url.searchParams.get('contact');
		if (param === null) return;
		untrack(() => void reveal(Number(param)));
	});

	async function reveal(id: number) {
		show(id);
		const url = new URL(page.url);
		url.searchParams.delete('contact');
		await page.goto(url.pathname + url.search, { replaceState: true });
	}
</script>

<PageTitle title="Contacts" />

<div class="px-(--page-inset)">
	<PageHeader title="Contacts">
		{#snippet actions()}
			<Button variant="secondary" onclick={() => (creating = true)}
				><Plus size={15} strokeWidth={1.75} aria-hidden="true" />New contact</Button
			>
		{/snippet}
	</PageHeader>

	{#if contacts.list.error && !contacts.list.loaded}
		<EmptyState {...workspaceLoadError(contacts.list.error, () => contacts.list.refetch())} />
	{:else if contacts.list.loaded}
		{#if contacts.list.value.length === 0}
			<EmptyState
				message="No contacts yet"
				actionLabel="New contact"
				onaction={() => (creating = true)}
			/>
		{:else}
			<div class="mb-5 max-w-80">
				<TextInput
					bind:value={query}
					type="search"
					placeholder="Filter by name"
					aria-label="Filter contacts by name"
					autocomplete="off"
				/>
			</div>

			{#if shown.length === 0}
				<EmptyState message="No matches" compact />
			{:else}
				<ul>
					{#each shown as contact (contact.id)}
						{@const first = contact.methods[0]}
						<li class="border-border not-first:border-t">
							<button
								class="group flex w-full cursor-pointer items-center justify-between gap-4 py-2 text-left"
								type="button"
								onclick={() => show(contact.id)}
							>
								<span class="flex min-w-0 items-center gap-3">
									<Photo photo={contact.photo} />
									<span class="min-w-0 truncate transition-colors group-hover:text-accent"
										>{fullName(contact)}</span
									>
								</span>
								{#if first}
									<span class="min-w-0 flex-none truncate text-xs text-fg-subtle max-sm:hidden"
										>{first.value}</span
									>
								{/if}
							</button>
						</li>
					{/each}
				</ul>
			{/if}
		{/if}
	{/if}
</div>

<ContactDialog bind:open={creating} workspaceId={ws} onsaved={(c) => show(c.id)} />

<ContactPanel bind:open={panelOpen} workspaceId={ws} contact={opened} />
