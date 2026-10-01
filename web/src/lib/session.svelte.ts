import { get } from './api';
import type { Computer, Me } from './types';

const KEY = 'cc_computer';

export const session = $state({
	me: null as Me | null,
	computerId: (typeof localStorage !== 'undefined' && localStorage.getItem(KEY)) || '',
	activityOpen: false,
	assistantOpen: false
});

export async function refreshMe(): Promise<Me> {
	const me = await get<Me>('/me');
	session.me = me;
	if (!me.computers.some((c) => c.id === session.computerId)) {
		setComputer(me.computers[0]?.id ?? '');
	}
	return me;
}

export function setComputer(id: string) {
	session.computerId = id;
	try {
		localStorage.setItem(KEY, id);
	} catch {
		// Private mode: the choice lasts for this tab.
	}
}

export function currentComputer(): Computer | null {
	return session.me?.computers.find((c) => c.id === session.computerId) ?? null;
}
