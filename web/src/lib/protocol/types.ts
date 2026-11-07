// The daemon's wire types. Everything the daemon defines comes from
// `generated.ts`, which `npm run generate` writes from cohesion-core; this
// file adds what only the client needs: request helpers, frames, and the
// CLI proxy's `/client` reply.
// Wire rules: `args` fields are snake_case; enums are camelCase except
// `IntelligenceTier`, which keeps its Rust variant names. Timestamps are unix
// seconds.

import type { Invalidation, NoArgs, Replies, Requests, ServerError, Vocabulary } from './generated';

export * from './generated';

// ---- Requests --------------------------------------------------------------

export type RequestType = keyof Requests;

/** The argument tuple of `send(type, ...args)`: empty for unit requests. */
export type ArgsOf<K extends RequestType> = Requests[K] extends NoArgs ? [] : [args: Requests[K]];

/** A function that runs one request and resolves with its reply data. */
export type Send = <K extends RequestType>(type: K, ...args: ArgsOf<K>) => Promise<Replies[K]>;

// ---- Frames ----------------------------------------------------------------

export interface WireRequest {
	id?: number;
	type: RequestType;
	args?: unknown;
}

export type ReplyOutcome = { ok: { type: string; data: unknown } } | { error: ServerError };

/** A request by name, for the API console: any `type`, unchecked `args`. */
export interface RawRequest {
	type: string;
	args?: unknown;
}

/** The whole reply to one request, success or protocol error. */
export type ReplyFrame = ReplyOutcome;

export type Frame =
	| { seq: number; type: 'hello'; protocol: number; server: string; vocabulary: Vocabulary }
	| ({ seq: number; type: 'reply'; id: number | null } & ReplyOutcome)
	| { seq: number; type: 'invalidate'; keys: Invalidation[] }
	| { seq: number; type: 'reset' }
	| { seq: number; type: 'evicted'; reason: EvictionReason };

/** Why the daemon ended the session; it is never reconnected after. This
 * page never offers, so only `revoked` reaches it. */
export type EvictionReason = 'replaced' | 'revoked';

/** `GET /client`: the workspace and target the CLI was opened with. */
export interface ClientInfo {
	workspace: number | null;
	target: string;
}
