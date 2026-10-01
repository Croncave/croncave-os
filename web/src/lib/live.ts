// Live updates from /api/live (server-sent events). Pages subscribe and refetch what changed.

export type LiveMsg = { account_id: string; kind: string; id: string; data: Record<string, unknown> };
type Listener = (m: LiveMsg) => void;

const listeners = new Set<Listener>();
let source: EventSource | null = null;

export function connectLive() {
	if (source) return;
	source = new EventSource('/api/live');
	source.addEventListener('live', (e) => {
		const msg = JSON.parse((e as MessageEvent).data) as LiveMsg;
		for (const l of listeners) l(msg);
	});
	source.addEventListener('resync', () => {
		for (const l of listeners) l({ account_id: '', kind: 'resync', id: '', data: {} });
	});
}

export function disconnectLive() {
	source?.close();
	source = null;
}

/** Subscribe; returns the unsubscribe function (use it as an $effect cleanup). */
export function onLive(fn: Listener): () => void {
	listeners.add(fn);
	return () => listeners.delete(fn);
}

/** Subscribe to changes worth reloading a page for (not every output line). */
export function onChange(fn: () => void): () => void {
	return onLive((m) => {
		if (m.kind !== 'output' && m.kind !== 'progress') fn();
	});
}

/** Call `fn` at most once per `ms` while messages keep arriving. */
export function throttle(fn: () => void, ms = 400): () => void {
	let timer: ReturnType<typeof setTimeout> | null = null;
	return () => {
		if (timer) return;
		timer = setTimeout(() => {
			timer = null;
			fn();
		}, ms);
	};
}
