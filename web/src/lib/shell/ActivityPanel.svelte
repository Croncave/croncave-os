<script lang="ts">
	import { get, post } from '$lib/api';
	import { onLive, throttle } from '$lib/live';
	import { session } from '$lib/session.svelte';
	import { ago, appName } from '$lib/format';
	import type { EventRow } from '$lib/types';
	import Status from '$lib/ui/Status.svelte';
	import Tabs from '$lib/ui/Tabs.svelte';

	// Activity as a side panel: everything every app reported, all or only unread.
	let { full = false }: { full?: boolean } = $props();
	let tab = $state('all');
	let events = $state<EventRow[]>([]);
	let showSystem = $state(false);

	async function load() {
		const r = await get(`/events?unread=${tab === 'unread'}&system=${showSystem}`);
		events = r.events;
		if (session.me) session.me.unread = r.unread;
	}
	$effect(() => {
		void tab;
		void showSystem;
		load();
	});
	$effect(() => onLive(throttle(() => load(), 300)));

	async function readAll() {
		await post('/events/read', { all: true });
		load();
	}
	async function open(e: EventRow) {
		if (!e.read_at) await post('/events/read', { ids: [e.id] });
		session.activityOpen = false;
		location.href = e.run_id ? `/runs/${e.run_id}` : e.app === 'billing' ? '/plans' : e.job_id ? `/${e.app}/${e.job_id}` : '/home';
	}
	const tone = (l: string) => ({ success: 'awake', needs_you: 'needs_you', failed: 'failed', warning: 'held', info: 'asleep', system: 'asleep' })[l] ?? 'asleep';
	const word = (l: string) => ({ success: 'Done', needs_you: 'Needs you', failed: 'Failed', warning: 'Heads up', info: 'Info', system: 'Computer' })[l] ?? l;
</script>

<aside class="activity" class:full aria-label="Activity" data-testid="activity">
	<div class="row head">
		<h2>Activity</h2><span class="spacer"></span>
		<button class="link" onclick={readAll}>Mark all read</button>
		{#if !full}<button class="link" onclick={() => (session.activityOpen = false)} aria-label="Close activity">✕</button>{/if}
	</div>
	<Tabs tabs={[{ id: 'all', label: 'All' }, { id: 'unread', label: `Unread${session.me?.unread ? ` (${session.me.unread})` : ''}` }]} bind:value={tab} />
	<div class="items">
		{#each events as e (e.id)}
			<button class="ev" class:unread={!e.read_at} onclick={() => open(e)}>
				<div class="row"><Status status={tone(e.level)} word={word(e.level)} /><span class="low">{appName(e.app)}</span><span class="spacer"></span><span class="low mono">{ago(e.created_at)}</span></div>
				<div class="title">{e.title}</div>
				{#if e.body}<div class="mid body">{e.body}</div>{/if}
				{#if e.actor_kind === 'assistant'}<div class="low">by the assistant</div>{/if}
			</button>
		{:else}
			<div class="muted-box">{tab === 'unread' ? "You're all caught up." : 'Nothing has happened yet.'}</div>
		{/each}
	</div>
	<label class="row low"><input type="checkbox" bind:checked={showSystem} /> Show when computers wake and sleep</label>
</aside>

<style>
	.activity {
		position: fixed;
		right: 0;
		top: 52px;
		bottom: 0;
		width: min(400px, 100vw);
		background: var(--surface);
		border-left: 1px solid var(--line);
		padding: 14px;
		display: grid;
		grid-template-rows: auto auto 1fr auto;
		gap: 10px;
		z-index: 25;
	}
	.full {
		position: static;
		width: auto;
		border: 0;
		background: none;
	}
	.items {
		overflow: auto;
		display: grid;
		align-content: start;
	}
	.ev {
		text-align: left;
		background: none;
		border: 0;
		border-bottom: 1px solid var(--line);
		padding: 10px 4px;
		color: var(--ink);
		font: inherit;
		cursor: pointer;
		display: grid;
		gap: 3px;
	}
	.ev:hover {
		background: var(--pane);
	}
	.unread .title {
		font-weight: 600;
	}
	.body {
		font-size: 13px;
	}
	.link {
		background: none;
		border: 0;
		color: var(--accent-ink);
		cursor: pointer;
		font: inherit;
	}
</style>
