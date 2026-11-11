// The daemon itself: what it is, its clients and pairing, security, storage, backups, the profile, workspaces, and the scripts it runs.

import { ProtocolError } from '../../errors';
import { noTransfers } from '../support';
import type { Handlers, MockCore } from '../core';

export function daemonHandlers(core: MockCore) {
	const {
		state,
		nextId,
		notFound,
		workspace,
		requireTitle,
		emojiValue,
		recordSecurity,
		pairingRefusal,
		revokeClient
	} = core;
	return {
		about: () => ({ hostname: 'mock.local', version: '0.0.0-mock' }),
		// The mock is the page itself, so like a daemon built without web/build it has no bundle.
		webBundle: () => {
			throw new ProtocolError(
				'badRequest',
				'This cohesiond was built without the web UI; run npm run build in web/ and rebuild it.'
			);
		},
		webBundleHash: () => ({ hash: null }),
		listWorkspaces: () => state.workspaces.filter((w) => !w.deleted).map((w) => ({ ...w })),
		listIntelligenceDefaults: () => state.defaults.map((d) => ({ ...d })),
		listClients: () =>
			state.clients
				.filter((c) => !c.deleted)
				.sort((a, b) =>
					a.client_name.localeCompare(b.client_name, undefined, { sensitivity: 'base' })
				)
				.map((c) => ({
					...c,
					online: state.onlineClients.includes(c.id),
					current: c.id === state.currentClient
				})),
		beginPairing: () => {
			const refusal = pairingRefusal();
			if (refusal) throw new ProtocolError('badRequest', refusal);
			const asking = state.clients.find((c) => c.id === state.currentClient);
			recordSecurity('pairingOpened', asking ?? null, asking?.last_address ?? null);
			return { code: btoa('mock-key|127.0.0.1|mock-fingerprint') };
		},
		getPairingAccess: () => ({ refusal: pairingRefusal() }),
		revokeOtherClients: () => {
			const others = state.clients.filter((c) => !c.deleted && c.id !== state.currentClient);
			return others.map((c) => revokeClient(c.id));
		},
		getSecuritySettings: () => ({ ...state.security }),
		updateSecuritySettings: (settings) => {
			state.security = { id: 1, ...settings };
			recordSecurity('settingsChanged', null);
			return { ...state.security };
		},
		getStorageSettings: () => ({ ...state.storage }),
		updateStorageSettings: (settings) => {
			if (Object.values(settings).some((value) => !(value >= 1)))
				throw new ProtocolError('validationFailed', 'A value is out of range.');
			state.storage = { id: 1, ...settings };
			return { ...state.storage };
		},
		getDatabaseSize: () => ({ ...state.databaseSize }),
		backupDatabase: () => ({
			path: '/home/you/.config/cohesiond/backups/cohesion-2026-11-03-120000.db',
			bytes: state.databaseSize.bytes
		}),
		// Moving data between daemons is the CLI's (`cohesion export` and
		// `cohesion import`); the web UI never sends these.
		exportDatabase: noTransfers,
		readDatabaseExport: noTransfers,
		beginDatabaseImport: noTransfers,
		writeDatabaseImport: noTransfers,
		finishDatabaseImport: noTransfers,
		getProfile: () => ({ ...state.profile }),
		setProfileName: ({ name }) => {
			state.profile.name = name?.trim() || null;
			return { ...state.profile };
		},
		setProfileBio: ({ bio }) => {
			const trimmed = bio?.trim() || null;
			if (trimmed && [...trimmed].length > 1000)
				throw new ProtocolError('validationFailed', 'Keep your bio to 1,000 characters.', 'bio');
			state.profile.bio = trimmed;
			return { ...state.profile };
		},
		setProfilePhoto: ({ photo }) => {
			state.profile.photo = photo
				? { kind: 'image', photo_media_type: photo.media_type, photo_data: photo.data }
				: { kind: 'none' };
			return { ...state.profile };
		},
		listSecurityEvents: ({ offset = 0, limit } = {}) => {
			const newest = [...state.securityEvents].reverse();
			return {
				total: newest.length,
				events: newest.slice(offset, offset + (limit ?? 50)).map((e) => ({ ...e }))
			};
		},
		listExtensions: () => state.extensions.map((e) => ({ ...e })),

		renameClient: ({ id, client_name }) => {
			const client = state.clients.find((c) => c.id === id && !c.deleted) ?? notFound('client');
			client.client_name = client_name.trim();
			recordSecurity('clientRenamed', client);
			return { ...client };
		},

		deleteClient: ({ id }) => revokeClient(id),

		createWorkspace: ({ title, emoji }) => {
			requireTitle(title);
			const created = {
				id: nextId(state.workspaces),
				title,
				emoji: emojiValue(emoji),
				deleted: false
			};
			state.workspaces.push(created);
			return { ...created };
		},
		renameWorkspace: ({ id, title }) => {
			const w = workspace(id);
			w.title = title;
			return { ...w };
		},
		setWorkspaceEmoji: ({ id, emoji }) => {
			const w = workspace(id);
			w.emoji = emojiValue(emoji);
			return { ...w };
		},
		deleteWorkspace: ({ id }) => {
			const w = workspace(id);
			w.deleted = true;
			return { ...w };
		},

		documentation: () => ({
			documentation: '// Mock documentation.\ndeclare const cohesion: {};\n'
		}),
		runScript: ({ script }) => JSON.stringify({ ran: script.length }),
		renderMermaid: ({ source }) =>
			/^\s*(flowchart|graph|sequenceDiagram)\b/.test(source)
				? {
						kind: 'svg',
						svg: '<svg xmlns="http://www.w3.org/2000/svg" class="pele" viewBox="0 0 120 40" width="120" height="40" style="max-width:100%;height:auto"><text x="60" y="24" text-anchor="middle" fill="var(--pele-fg,#1f1f1f)">Mock diagram</text></svg>',
						width: 120,
						height: 40
					}
				: { kind: 'refused', reason: 'syntax', message: 'Mock: this is not a diagram.' }
	} satisfies Partial<Handlers>;
}
