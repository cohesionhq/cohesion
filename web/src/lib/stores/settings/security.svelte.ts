// Settings → Security: who can add clients, alerts, revoking clients that
// stopped connecting, and the security activity. Every part refetches on
// the `security` key.

import { send as defaultSend } from '$lib/protocol/api';
import type {
	Invalidation,
	SecurityEventPage,
	SecuritySettings,
	Send,
	UpdateSecuritySettingsRequest
} from '$lib/protocol/types';
import { Resource } from '../resource.svelte';

/** The settings changed, or something was added to the activity. */
const securityChanged = (key: Invalidation) => key.kind === 'security';

export class SecurityStore {
	readonly settings: Resource<SecuritySettings | null>;
	/** One page of the activity, keyed by how many events come before it. */
	readonly activity: Resource<SecurityEventPage, number>;
	/** How many events a page of the activity holds. */
	pageSize = $state(50);

	#send: Send;

	constructor(send: Send) {
		this.#send = send;
		this.settings = new Resource({
			fetch: () => send('getSecuritySettings'),
			initial: null,
			matches: securityChanged
		});
		this.activity = new Resource({
			fetch: (offset: number) => send('listSecurityEvents', { offset, limit: this.pageSize }),
			initial: { total: 0, events: [] },
			matches: securityChanged
		});
	}

	/** Loads the settings, and the activity in pages of `pageSize`. */
	async load(pageSize = this.pageSize): Promise<void> {
		this.pageSize = pageSize;
		await Promise.all([this.settings.load(), this.activity.load(0)]);
	}

	release(): void {
		this.settings.release();
		this.activity.release();
	}

	/** Shows the activity from `offset`. */
	page(offset: number): Promise<void> {
		return this.activity.load(offset);
	}

	/** Shows pages of `limit` events, from the newest. */
	resize(limit: number): Promise<void> {
		this.pageSize = limit;
		return this.activity.key === 0 ? this.activity.refetch() : this.activity.load(0);
	}

	/** Changes some of the settings, keeping the rest. */
	async update(change: Partial<UpdateSecuritySettingsRequest>): Promise<SecuritySettings> {
		const current = this.settings.value;
		if (!current) throw new Error('The security settings haven’t loaded.');
		const updated = await this.#send('updateSecuritySettings', {
			pairing_opened_by: current.pairing_opened_by,
			alert_new_client: current.alert_new_client,
			revoke_inactive_after_days: current.revoke_inactive_after_days,
			...change
		});
		this.settings.update(() => updated);
		return updated;
	}
}

export function createSecurityStore(send: Send = defaultSend): SecurityStore {
	return new SecurityStore(send);
}

export const security = createSecurityStore();
