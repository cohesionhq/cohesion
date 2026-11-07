import { describe, expect, it } from 'vitest';
import { createMock } from '$lib/protocol/mock';
import { refreshInvalidated } from '../resource.svelte';
import { createSecurityStore } from './security.svelte';

describe('security store', () => {
	it('changes one setting and keeps the rest', async () => {
		const mock = createMock();
		const store = createSecurityStore(mock.send);
		await store.load();
		const before = store.settings.value!;

		await store.update({ pairing_opened_by: 'localClients' });
		expect(store.settings.value).toEqual({ ...before, pairing_opened_by: 'localClients' });
		expect(mock.state.security.pairing_opened_by).toBe('localClients');
	});

	it('pages the activity newest first and refetches it when security changes', async () => {
		const mock = createMock();
		const store = createSecurityStore(mock.send);
		await store.load(2);
		const total = store.activity.value.total;
		expect(store.activity.value.events).toHaveLength(2);
		const [newest, next] = store.activity.value.events;
		expect(newest.created_at).toBeGreaterThanOrEqual(next.created_at);

		await store.page(2);
		expect(store.activity.key).toBe(2);
		await store.update({ alert_new_client: false });
		await store.page(0);
		await refreshInvalidated([{ kind: 'security' }]);
		expect(store.activity.value.total).toBe(total + 1);
		expect(store.activity.value.events[0].kind).toBe('settingsChanged');
	});
});
