// An in-memory daemon implementing every request kind, for `VITE_MOCK=1` and
// as the fixture for tests. Each `createMock()` call starts from fresh seed data.
//
// It is in parts under `mock/`: `seed.ts` is the data it starts from,
// `core.ts` its state and the lookups every area shares, `handlers/` one file
// of request handlers per area, and `tableRules.ts` the daemon's rules for
// table values, formulas, filters and sorts. A new request kind gets its
// handler in its area's file; `createMock` fails to compile until every kind
// has one.

import { toProtocolError } from './errors';
import { createCore, type Handlers } from './mock/core';
import { daemonHandlers } from './mock/handlers/daemon';
import { documentsHandlers } from './mock/handlers/documents';
import { tablesHandlers } from './mock/handlers/tables';
import { rewindHandlers } from './mock/handlers/rewind';
import { calendarsHandlers } from './mock/handlers/calendars';
import { contactsHandlers } from './mock/handlers/contacts';
import { tasksHandlers } from './mock/handlers/tasks';
import { linksHandlers } from './mock/handlers/links';
import { intelligenceHandlers } from './mock/handlers/intelligence';
import { extensionsHandlers } from './mock/handlers/extensions';
import { connectorsHandlers } from './mock/handlers/connectors';
import { feedsHandlers } from './mock/handlers/feeds';
import { agentsHandlers } from './mock/handlers/agents';
import { actorsHandlers } from './mock/handlers/actors';
import type { MockState } from './mock/state';
import type {
	ArgsOf,
	Invalidation,
	RawRequest,
	Replies,
	ReplyFrame,
	Requests,
	RequestType,
	Send
} from './types';

export { MOCK_VOCABULARY } from './mock/support';
export type { TranscriptItem } from './mock/state';

export interface Mock {
	send: Send;
	sendRaw: (request: RawRequest) => Promise<ReplyFrame>;
	state: MockState;
	/** Hears the invalidations the mock pushes after a write; returns an unsubscribe function. */
	onInvalidate: (listener: (keys: Invalidation[]) => void) => () => void;
}

export function createMock(): Mock {
	const core = createCore();
	const handlers: Handlers = {
		...daemonHandlers(core),
		...documentsHandlers(core),
		...tablesHandlers(core),
		...rewindHandlers(core),
		...contactsHandlers(core),
		...calendarsHandlers(core),
		...tasksHandlers(core),
		...linksHandlers(core),
		...intelligenceHandlers(core),
		...extensionsHandlers(core),
		...connectorsHandlers(core),
		...feedsHandlers(core),
		...agentsHandlers(core),
		...actorsHandlers(core)
	};

	const send = (<K extends RequestType>(type: K, ...args: ArgsOf<K>): Promise<Replies[K]> => {
		try {
			const handler = handlers[type] as (args: Requests[K] | undefined) => Replies[K];
			return Promise.resolve(handler(args[0] as Requests[K] | undefined));
		} catch (error) {
			return Promise.reject(error);
		}
	}) as Send;

	// Rebuilds the frame the daemon would send, error replies included.
	const sendRaw = async ({ type, args }: RawRequest): Promise<ReplyFrame> => {
		const handler = (handlers as Record<string, ((args: unknown) => unknown) | undefined>)[type];
		if (!handler)
			return { error: { code: 'badRequest', message: `Unknown request type ${type}.` } };
		try {
			return { ok: { type, data: handler(args) } };
		} catch (error) {
			const e = toProtocolError(error);
			const code = e.code === 'disconnected' ? 'internalError' : e.code;
			return { error: { code, message: e.message, ...(e.field ? { field: e.field } : {}) } };
		}
	};

	const onInvalidate = (listener: (keys: Invalidation[]) => void) => {
		core.invalidateListeners.add(listener);
		return () => core.invalidateListeners.delete(listener);
	};

	return { send, sendRaw, state: core.state, onInvalidate };
}
