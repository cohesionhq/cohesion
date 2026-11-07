// Settings → Clients: the clients registered with the daemon, each saying
// whether it is connected now. This list is the one place presence is read:
// a client's actors are online while their client is.

import { send as defaultSend } from '$lib/protocol/api';
import type {
	Client,
	Invalidation,
	ListedClient,
	PairingAccess,
	PairingCode,
	Send
} from '$lib/protocol/types';
import { Resource } from '../resource.svelte';

/** The first 12 hex digits of a key's fingerprint, enough to tell clients apart. */
export function shortKey(key: string): string {
	const hex = key.replace(/[^0-9a-f]/gi, '').toLowerCase();
	return hex.length > 12 ? `${hex.slice(0, 12)}…` : hex;
}

/** Registering, renaming or revoking a client, and one connecting or disconnecting. */
const clientsChanged = (key: Invalidation) => key.kind === 'clients';

/** The security settings changed, which decide whether this client may open pairing. */
const securityChanged = (key: Invalidation) => key.kind === 'security';

export class ClientsStore {
	readonly list: Resource<ListedClient[]>;
	/** Whether this client may open pairing now, as the daemon says. */
	readonly pairing: Resource<PairingAccess>;

	#send: Send;

	constructor(send: Send) {
		this.#send = send;
		this.list = new Resource({
			fetch: () => send('listClients'),
			initial: [],
			matches: clientsChanged
		});
		this.pairing = new Resource({
			fetch: () => send('getPairingAccess'),
			initial: { refusal: null },
			matches: securityChanged
		});
	}

	async load(): Promise<void> {
		await Promise.all([this.list.load(), this.pairing.load()]);
	}

	release(): void {
		this.list.release();
		this.pairing.release();
	}

	/** Whether client `id` is connected now, as the list says. */
	isOnline(id: number): boolean {
		return this.list.value.some((client) => client.id === id && client.online);
	}

	/** The client this page is, once the list has it. */
	get current(): ListedClient | undefined {
		return this.list.value.find((client) => client.current);
	}

	/** Client `id`'s name, once the list has it. */
	name(id: number): string | undefined {
		return this.list.value.find((client) => client.id === id)?.client_name;
	}

	/** Opens a pairing window: the setup code a new client registers with, good once for ten minutes. */
	beginPairing(): Promise<PairingCode> {
		return this.#send('beginPairing');
	}

	/** Renames a client. */
	async rename(id: number, client_name: string): Promise<Client> {
		const renamed = await this.#send('renameClient', { id, client_name });
		this.list.update((list) => list.map((c) => (c.id === id ? { ...c, ...renamed } : c)));
		return renamed;
	}

	/** Revokes every client but this one, and answers how many went. */
	async revokeOthers(): Promise<number> {
		const revoked = await this.#send('revokeOtherClients');
		const gone = revoked.map((client) => client.id);
		this.list.update((list) => list.filter((c) => !gone.includes(c.id)));
		return revoked.length;
	}

	/** Revokes a client; its certificate is no longer trusted. */
	async revoke(id: number): Promise<Client> {
		const revoked = await this.#send('deleteClient', { id });
		this.list.update((list) => list.filter((c) => c.id !== revoked.id));
		return revoked;
	}
}

export function createClientsStore(send: Send = defaultSend): ClientsStore {
	return new ClientsStore(send);
}

export const clients = createClientsStore();
