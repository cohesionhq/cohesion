// Connectors: services, accounts and the resources imported through them.

import { ProtocolError } from '../../errors';
import type { Resource } from '../../types';
import { nowSeconds } from '../support';
import type { Handlers, MockCore } from '../core';

export function connectorsHandlers(core: MockCore) {
	const {
		state,
		nextId,
		notFound,
		workspace,
		shownKinds,
		granted,
		connector,
		connectorAccount,
		definition,
		redacted,
		served,
		resource,
		remoteResource
	} = core;
	return {
		// Each extension granted `oauth` describes itself; a failing one is
		// named after the extension and carries the error.
		listConnectorServices: () =>
			state.extensions
				.filter((e) => granted(e.id, 'oauth'))
				.map((e) => {
					const c = state.connectors.find((x) => x.extension_id === e.id);
					const error = c ? c.error : `${e.name}: serviceDescription is not defined`;
					return {
						extension_id: e.id,
						name: e.name,
						title: c && !error ? c.title : e.name,
						unit: c && !error ? c.unit : '',
						icon: e.icon,
						error,
						sign_in_hint: null
					};
				}),
		beginConnectorAuthentication: ({ extension_id }) => {
			connector(extension_id, ['oauth']);
			return { url: `https://example.com/connect/${extension_id}/authorize` };
		},
		// Any code signs in to the service's one account; adding it again replaces it.
		completeConnectorAuthentication: ({ extension_id, code }) => {
			const c = connector(extension_id, ['oauth']);
			if (code.trim() === '')
				throw new ProtocolError('upstreamError', `${c.title}: the code was refused.`);
			const extension = state.extensions.find((e) => e.id === extension_id)!;
			// The definition is stored the first time, as the daemon does.
			let stored = state.definitions.find((d) => d.identifier === extension.script_identifier);
			if (!stored) {
				stored = {
					id: nextId(state.definitions),
					identifier: extension.script_identifier,
					version: 1,
					title: c.title,
					icon: extension.icon,
					unit: c.unit,
					kinds: [],
					metadata_keys: [],
					created_at: nowSeconds()
				};
				const id = stored.id;
				let kindId = Math.max(0, ...state.definitions.flatMap((d) => d.kinds.map((k) => k.id)));
				stored.kinds = c.kinds.map(([identifier, title, plural]) => ({
					id: ++kindId,
					connector_id: id,
					identifier,
					title,
					plural,
					contents: ''
				}));
				state.definitions.push(stored);
			}
			let account = state.connectorAccounts.find(
				(a) =>
					a.host.kind === 'extension' &&
					a.host.extension_id === extension_id &&
					a.host.identifier === c.account.identifier
			);
			if (!account) {
				account = {
					id: nextId(state.connectorAccounts),
					connector_id: stored.id,
					host: { kind: 'extension', extension_id, identifier: c.account.identifier },
					description: '',
					credential_failed_at: null,
					created_at: nowSeconds()
				};
				state.connectorAccounts.push(account);
			}
			account.description = c.account.description;
			// A new credential clears the flag, as the daemon does.
			account.credential_failed_at = null;
			return redacted(account);
		},
		listConnectorAccounts: () =>
			state.connectorAccounts
				.map(redacted)
				.sort(
					(a, b) =>
						a.description.localeCompare(b.description, undefined, { sensitivity: 'base' }) ||
						a.connector.title.localeCompare(b.connector.title, undefined, { sensitivity: 'base' })
				),
		deleteConnectorAccount: ({ account_id }) => {
			const account = redacted(connectorAccount(account_id));
			state.connectorAccounts = state.connectorAccounts.filter((a) => a.id !== account_id);
			state.resources = state.resources.filter((r) => r.account_id !== account_id);
			return account;
		},
		// Within one connector; the target's copy of an item both have stays.
		migrateResources: ({ from_account_id, to_account_id }) => {
			const from = connectorAccount(from_account_id);
			const to = connectorAccount(to_account_id);
			if (from.connector_id !== to.connector_id)
				throw new ProtocolError('notFound', 'This operation contains an invalid reference.');
			const held = (r: Resource) =>
				state.resources.some(
					(t) =>
						t.account_id === to_account_id &&
						t.workspace_id === r.workspace_id &&
						t.identifier === r.identifier
				);
			state.resources = state.resources.filter((r) => r.account_id !== from_account_id || !held(r));
			for (const r of state.resources)
				if (r.account_id === from_account_id) r.account_id = to_account_id;
			return null;
		},
		listConnectorOrganizationUnits: ({ account_id }) => {
			const account = connectorAccount(account_id);
			served(account);
			if (account.host.kind !== 'extension') throw new Error('served() refuses an orphan');
			return connector(account.host.extension_id).units.map((u) => ({ ...u }));
		},
		searchConnectorResources: ({
			account_id,
			workspace_id,
			kind_id,
			organization_unit_id,
			search
		}) => {
			workspace(workspace_id);
			const account = connectorAccount(account_id);
			served(account);
			const kind = definition(account.connector_id).kinds.find((k) => k.id === kind_id);
			if (!kind) throw new ProtocolError('validationFailed', 'No such kind.');
			const q = search?.toLowerCase() ?? '';
			const { host } = account;
			if (host.kind !== 'extension') throw new Error('served() refuses an orphan');
			return connector(host.extension_id)
				.resources.filter((r) => !organization_unit_id || r.unit === organization_unit_id)
				.filter((r) => r.kind_identifier === kind.identifier)
				.filter((r) => r.title.toLowerCase().includes(q))
				.map((r) => ({
					identifier: r.identifier,
					title: r.title,
					// A date the service doesn't have is null, as the daemon sends it.
					last_updated: r.last_updated || null,
					imported: state.resources.some(
						(a) =>
							a.workspace_id === workspace_id &&
							a.account_id === account_id &&
							a.identifier === r.identifier
					)
				}));
		},
		// Upserts by (workspace, account, identifier); `kind` is prefixed with
		// the connector's identifier, as a read of it is.
		importResource: ({ account_id, workspace_id, identifier }) => {
			workspace(workspace_id);
			const account = connectorAccount(account_id);
			const remote = remoteResource(account, identifier);
			const kind = definition(account.connector_id).kinds.find(
				(k) => k.identifier === remote.kind_identifier
			);
			if (!kind)
				throw new ProtocolError(
					'validationFailed',
					`The connector doesn’t declare the kind "${remote.kind_identifier}".`
				);
			let imported = state.resources.find(
				(a) =>
					a.workspace_id === workspace_id &&
					a.account_id === account_id &&
					a.identifier === identifier
			);
			if (!imported) {
				imported = {
					id: nextId(state.resources),
					workspace_id,
					account_id,
					identifier,
					kind_id: kind.id,
					kind: '',
					title: '',
					content_kind: remote.content_kind,
					contents: '',
					last_updated: 0,
					modified: 0
				};
				state.resources.push(imported);
			}
			Object.assign(imported, {
				kind_id: kind.id,
				kind: remote.kind,
				title: remote.title,
				content_kind: remote.content_kind,
				contents: remote.contents,
				last_updated: remote.last_updated,
				modified: nowSeconds()
			});
			return { ...imported };
		},
		// The mock keeps no metadata, so a metadata sort leaves the newest import first.
		listResources: ({ workspace_id, kind, search, sort, descending }) => {
			workspace(workspace_id);
			const q = search?.toLowerCase() ?? '';
			return state.resources
				.filter(
					(a) =>
						a.workspace_id === workspace_id &&
						(kind == null || a.kind === kind) &&
						a.title.toLowerCase().includes(q)
				)
				.sort((a, b) => {
					const order =
						sort === 'title'
							? a.title.localeCompare(b.title, undefined, { sensitivity: 'base' })
							: sort === 'lastUpdated'
								? a.last_updated - b.last_updated
								: 0;
					return (descending ? -order : order) || b.id - a.id;
				})
				.map((a) => ({ ...a }));
		},
		getResource: ({ workspace_id, resource_id }) => ({ ...resource(workspace_id, resource_id) }),
		// Like the daemon, a resource whose connector can't draw it has no view.
		renderResource: ({ workspace_id, resource_id }) => {
			const found = resource(workspace_id, resource_id);
			const { host } = connectorAccount(found.account_id);
			if (host.kind !== 'extension' || !granted(host.extension_id, 'connector')) return null;
			const service = state.connectors.find((c) => c.extension_id === host.extension_id);
			const remote = service?.resources.find((r) => r.identifier === found.identifier);
			return remote?.render ? structuredClone(remote.render) : null;
		},
		deleteResource: ({ workspace_id, resource_id }) => {
			const removed = resource(workspace_id, resource_id);
			state.resources = state.resources.filter((a) => a.id !== resource_id);
			return { ...removed };
		},
		listWorkspaceResourceKinds: ({ workspace_id }) => {
			workspace(workspace_id);
			return shownKinds(workspace_id);
		},
		setResourceKindShown: ({ workspace_id, kind_id, shown }) => {
			workspace(workspace_id);
			const inWorkspace = state.shownKinds.filter((k) => k.workspace_id === workspace_id);
			const at = state.shownKinds.findIndex(
				(k) => k.workspace_id === workspace_id && k.kind_id === kind_id
			);
			// Like the daemon, a kind shown later goes last.
			if (shown && at === -1)
				state.shownKinds.push({
					id: nextId(state.shownKinds),
					workspace_id,
					kind_id,
					sort_order: Math.max(0, ...inWorkspace.map((k) => k.sort_order)) + 1
				});
			if (!shown && at !== -1) state.shownKinds.splice(at, 1);
			return shownKinds(workspace_id);
		},
		reorderWorkspaceResourceKinds: ({ workspace_id, kind_ids }) => {
			workspace(workspace_id);
			kind_ids.forEach((kind_id, i) => {
				const shown = state.shownKinds.find(
					(k) => k.workspace_id === workspace_id && k.kind_id === kind_id
				);
				(shown ?? notFound('kind')).sort_order = i;
			});
			return shownKinds(workspace_id);
		},
		// Takes the service's copy of each resource whose `last_updated` moved.
		refreshResources: ({ workspace_id }) => {
			workspace(workspace_id);
			const summary = { checked: 0, updated: 0, failed: 0 };
			// Like the daemon, a resource counts as checked only when its fetch
			// succeeds, and an orphaned account's are skipped.
			for (const a of state.resources.filter((x) => x.workspace_id === workspace_id)) {
				const account = connectorAccount(a.account_id);
				if (account.host.kind === 'orphaned') continue;
				try {
					const remote = remoteResource(account, a.identifier);
					summary.checked++;
					if (remote.last_updated === a.last_updated) continue;
					Object.assign(a, {
						title: remote.title,
						contents: remote.contents,
						content_kind: remote.content_kind,
						last_updated: remote.last_updated,
						modified: nowSeconds()
					});
					summary.updated++;
				} catch {
					summary.failed++;
				}
			}
			return summary;
		}
	} satisfies Partial<Handlers>;
}
