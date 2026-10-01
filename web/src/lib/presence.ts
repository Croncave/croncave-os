import { post } from './api';
import { refreshMe } from './session.svelte';

/**
 * While this screen is open and visible, tell the platform: it keeps the computer awake
 * (and wakes it). Only screens that read the computer itself use this (Files, Code,
 * previews); screens that show stored results let it sleep.
 */
export function keepAwake(computer: () => string, cause: 'files' | 'app' | 'preview', app: string): () => void {
	const ping = async () => {
		const id = computer();
		if (!id || document.visibilityState !== 'visible') return;
		try {
			const r = await post(`/computers/${id}/presence`, { cause, app });
			if (r.state === 'waking' || r.state === 'asleep') refreshMe();
		} catch {
			// The page shows its own errors.
		}
	};
	ping();
	const t = setInterval(ping, 15_000);
	const vis = () => ping();
	document.addEventListener('visibilitychange', vis);
	return () => {
		clearInterval(t);
		document.removeEventListener('visibilitychange', vis);
	};
}
