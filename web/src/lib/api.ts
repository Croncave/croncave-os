import { goto } from '$app/navigation';

export class ApiError extends Error {
	constructor(
		message: string,
		public status: number,
		public code: string | null
	) {
		super(message);
	}
}

type Opts = { method?: string; body?: unknown; raw?: BodyInit; actor?: 'assistant'; signal?: AbortSignal };

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export async function api<T = any>(path: string, opts: Opts = {}): Promise<T> {
	const headers: Record<string, string> = {};
	if (opts.body !== undefined) headers['content-type'] = 'application/json';
	if (opts.actor) headers['x-croncave-actor'] = opts.actor;
	const res = await fetch(`/api${path}`, {
		method: opts.method ?? (opts.body !== undefined || opts.raw !== undefined ? 'POST' : 'GET'),
		headers,
		body: opts.raw ?? (opts.body !== undefined ? JSON.stringify(opts.body) : undefined),
		signal: opts.signal
	});
	if (!res.ok) {
		const j = await res.json().catch(() => ({}));
		if (res.status === 401 && !path.startsWith('/auth') && !location.pathname.startsWith('/signin')) {
			await goto('/signin');
		}
		throw new ApiError(j.error ?? res.statusText, res.status, j.code ?? null);
	}
	return res.json();
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export const get = <T = any>(path: string) => api<T>(path);
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export const post = <T = any>(path: string, body: unknown = {}, actor?: 'assistant') => api<T>(path, { body, actor });
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export const patch = <T = any>(path: string, body: unknown) => api<T>(path, { method: 'PATCH', body });
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export const del = <T = any>(path: string) => api<T>(path, { method: 'DELETE' });

export function message(e: unknown): string {
	return e instanceof Error ? e.message : String(e);
}
