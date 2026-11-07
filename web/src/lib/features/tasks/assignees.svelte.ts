// Who tasks are assigned to, from the agents every page shares, the task's
// workspace's contacts, which the caller holds, and the user's profile,
// which only loads once something shows the user's avatar.

import type { Contact, TaskAssignee } from '$lib/protocol/types';
import { mainStores } from '$lib/stores/pageStores.svelte';
import { profile } from '$lib/stores/settings/profile.svelte';
import { assigneeChoices, shownAssignee } from './assignee';

const { agents } = mainStores;

export const assignees = {
	/** Who `assignee` is as the app shows them; null for no one. */
	shown: (assignee: TaskAssignee | null, contacts: Contact[]) =>
		shownAssignee(assignee, agents.list.value, contacts, profile.profile.value),
	/** Who a task can be assigned to: the user, then each agent, then each of `contacts`. */
	choices: (contacts: Contact[]) =>
		assigneeChoices(agents.list.value, contacts, profile.profile.value),
	/** Fetches the profile the user's avatar is drawn from, once. */
	loadProfile: () => void profile.load()
};
