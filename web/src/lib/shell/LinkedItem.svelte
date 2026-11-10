<!-- The item a link or a forward opened, in its kind's panel over whatever page it was on (`linkedItem`). -->
<script lang="ts">
	import ActorSessionPanel from '$lib/features/actors/ActorSessionPanel.svelte';
	import AgentPanel from '$lib/features/agents/AgentPanel.svelte';
	import ArticlePanel from '$lib/features/articles/ArticlePanel.svelte';
	import ResourcePanel from '$lib/features/connectors/ResourcePanel.svelte';
	import ContactPanel from '$lib/features/contacts/ContactPanel.svelte';
	import DatabaseRowPanel from '$lib/features/databases/DatabaseRowPanel.svelte';
	import DocumentPanel from '$lib/features/documents/DocumentPanel.svelte';
	import TaskDetails from '$lib/features/tasks/TaskDetails.svelte';
	import { mainStores } from '$lib/stores/pageStores.svelte';
	import { untrack } from 'svelte';
	import { linkedItem } from './linkedItem.svelte';

	const item = $derived(linkedItem.item);

	// A contact's panel changes it through its workspace's contacts, so they
	// load while it shows, and it shows the contact as they hold it.
	const { contacts } = mainStores;
	const contactWorkspace = $derived(item?.kind === 'contact' ? item.contact.workspace_id : null);
	$effect(() => {
		const ws = contactWorkspace;
		if (ws === null) return;
		untrack(() => void contacts.load(ws));
		return () => contacts.list.release();
	});
	const contact = $derived(
		item?.kind === 'contact'
			? (contacts.list.value.find((c) => c.id === item.contact.id) ?? item.contact)
			: null
	);
</script>

{#if item?.kind === 'task'}
	<TaskDetails
		bind:open={linkedItem.open}
		task={item.task}
		folders={item.folders}
		tags={item.tags}
		contacts={item.contacts}
	/>
{:else if item?.kind === 'document'}
	<DocumentPanel bind:open={linkedItem.open} document={item.document} />
{:else if item?.kind === 'resource'}
	<ResourcePanel bind:open={linkedItem.open} resource={item.resource} />
{:else if item?.kind === 'contact'}
	<ContactPanel bind:open={linkedItem.open} workspaceId={item.contact.workspace_id} {contact} />
{:else if item?.kind === 'agent'}
	<AgentPanel bind:open={linkedItem.open} agent={item.agent} />
{:else if item?.kind === 'actor-session'}
	<ActorSessionPanel
		bind:open={linkedItem.open}
		session={item.session}
		actor={item.actor}
		messages={item.messages}
	/>
{:else if item?.kind === 'article'}
	<ArticlePanel bind:open={linkedItem.open} article={item.article} feedTitle={item.feedTitle} />
{:else if item?.kind === 'database-row'}
	<DatabaseRowPanel
		bind:open={linkedItem.open}
		ws={item.ws}
		database={item.database}
		row={item.row}
		label={item.label}
	/>
{/if}
