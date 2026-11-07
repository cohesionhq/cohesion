import type { ErrorCode, ServerError } from './types';

/** A failed request: the daemon's error reply, or a transport failure mapped onto one. */
export class ProtocolError extends Error {
	readonly code: ErrorCode | 'disconnected';
	readonly field?: string;

	constructor(code: ErrorCode | 'disconnected', message: string, field?: string) {
		super(message);
		this.name = 'ProtocolError';
		this.code = code;
		this.field = field;
	}

	static from(error: ServerError): ProtocolError {
		return new ProtocolError(error.code, error.message, error.field ?? undefined);
	}
}

/** Wraps anything thrown into a `ProtocolError` so the UI shows one shape. */
export function toProtocolError(error: unknown): ProtocolError {
	if (error instanceof ProtocolError) return error;
	const message = error instanceof Error ? error.message : String(error);
	return new ProtocolError('internalError', message);
}
