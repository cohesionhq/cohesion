// Connectors: the accounts added under Connectors → Accounts, each with the
// connector it is one of, and the calls into extensions that add accounts
// (`oauth`) and browse what they offer (`connector`). An orphaned account
// neither browses nor imports, until its resources move to another account.

import type {
	Connector,
	ConnectorAuthentication,
	ConnectorService,
	ImportableResource,
	OrganizationUnit,
	RedactedConnectorAccount,
	Resource as ImportedResource,
	ResourceKind,
	Send
} from '$lib/protocol/types';
import { Resource } from './resource.svelte';
import { send as defaultSend } from '$lib/protocol/api';

export interface ResourceSearch {
	account_id: number;
	workspace_id: number;
	kind_id: number;
	organization_unit_id: string | null;
	search: string | null;
}

/** A kind of resource and the connector that declares it. */
export interface ConnectorKind {
	connector: Connector;
	kind: ResourceKind;
}

export class ConnectorsStore {
	readonly accounts: Resource<RedactedConnectorAccount[]>;

	#send: Send;

	constructor(send: Send) {
		this.#send = send;
		this.accounts = new Resource({
			fetch: () => send('listConnectorAccounts'),
			initial: [],
			matches: (key) => key.kind === 'connectorAccounts'
		});
	}

	load(): Promise<void> {
		return this.accounts.load();
	}

	/** The account `id`, if it is loaded. */
	account(id: number): RedactedConnectorAccount | undefined {
		return this.accounts.value.find((a) => a.id === id);
	}

	/**
	 * Every connector with an account whose items can be browsed and
	 * imported, once each, in the accounts' order. An extension's account
	 * that is only a sign-in, or an orphan, doesn't count.
	 */
	get connectors(): Connector[] {
		return this.accounts.value
			.filter((a) => a.importable)
			.map((a) => a.connector)
			.filter((c, i, all) => all.findIndex((other) => other.id === c.id) === i);
	}

	/**
	 * Every kind of every connector in `connectors`, in their order: what a
	 * workspace can show.
	 */
	get kinds(): ConnectorKind[] {
		return this.connectors.flatMap((connector) =>
			connector.kinds.map((kind) => ({ connector, kind }))
		);
	}

	/** Kind `id` and its connector, if one of its accounts is loaded. */
	kind(id: number): ConnectorKind | undefined {
		for (const { connector } of this.accounts.value) {
			const kind = connector.kinds.find((k) => k.id === id);
			if (kind) return { connector, kind };
		}
		return undefined;
	}

	/** The other accounts of `account`'s connector, which its resources can move to. */
	siblings(account: RedactedConnectorAccount): RedactedConnectorAccount[] {
		return this.accounts.value.filter(
			(a) => a.id !== account.id && a.connector.id === account.connector.id
		);
	}

	/** What people call `resource`'s kind, once its account is loaded. */
	kindTitle(resource: ImportedResource): string {
		return this.kind(resource.kind_id)?.kind.title ?? resource.kind;
	}

	/**
	 * Every extension granted `oauth`, as it describes itself. Each is a
	 * script, so this goes over HTTP; one that fails carries `error`.
	 */
	services(): Promise<ConnectorService[]> {
		return this.#send('listConnectorServices');
	}

	/** The sign-in page to open for a new account with `extension_id`. */
	beginAuthentication(extension_id: number): Promise<ConnectorAuthentication> {
		return this.#send('beginConnectorAuthentication', { extension_id });
	}

	/**
	 * Hands the code from the sign-in page to the extension and adds (or, for
	 * the same remote account, replaces) the account.
	 */
	async completeAuthentication(
		extension_id: number,
		code: string
	): Promise<RedactedConnectorAccount> {
		const account = await this.#send('completeConnectorAuthentication', { extension_id, code });
		this.accounts.update((list) =>
			list.some((a) => a.id === account.id)
				? list.map((a) => (a.id === account.id ? account : a))
				: [...list, account]
		);
		return account;
	}

	/**
	 * Removes an account; the daemon removes what it imported from every
	 * workspace.
	 */
	async remove(account_id: number): Promise<void> {
		const removed = await this.#send('deleteConnectorAccount', { account_id });
		this.accounts.update((list) => list.filter((a) => a.id !== removed.id));
	}

	/**
	 * Moves every resource of account `from` to account `to`, of the same
	 * connector, in every workspace.
	 */
	async migrate(from: number, to: number): Promise<void> {
		await this.#send('migrateResources', { from_account_id: from, to_account_id: to });
	}

	/** The groups (workspaces, teams…) an account's records are filed under. */
	organizationUnits(account_id: number): Promise<OrganizationUnit[]> {
		return this.#send('listConnectorOrganizationUnits', { account_id });
	}

	/** What an account offers to import of one kind, marking what `workspace_id` already has. */
	searchResources(args: ResourceSearch): Promise<ImportableResource[]> {
		return this.#send('searchConnectorResources', args);
	}

	/** Copies one record into `workspace_id` as a resource; a client must be online for it. */
	importResource(
		account_id: number,
		workspace_id: number,
		identifier: string
	): Promise<ImportedResource> {
		return this.#send('importResource', { account_id, workspace_id, identifier });
	}
}

export function createConnectorsStore(send: Send = defaultSend): ConnectorsStore {
	return new ConnectorsStore(send);
}

export const connectors = createConnectorsStore();
