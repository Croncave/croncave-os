<script lang="ts">
	import { goto } from '$app/navigation';
	import { get, post } from '$lib/api';
	import { onChange, throttle } from '$lib/live';
	import { session } from '$lib/session.svelte';
	import { appIcon, clock } from '$lib/format';
	import type { EventRow } from '$lib/types';
	import Icon from '$lib/ui/Icon.svelte';

	// Activity: everything every app reported, all or only unread, grouped by day. A side
	// panel next to the app, or (full) the whole history page.
	let { full = false }: { full?: boolean } = $props();
	let tab = $state<'all' | 'unread'>('all');
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
	$effect(() => onChange(throttle(() => load(), 300)));

	async function readAll() {
		await post('/events/read', { all: true });
		load();
	}
	async function open(e: EventRow) {
		if (!e.read_at) await post('/events/read', { ids: [e.id] });
		if (!full) session.activityOpen = false;
		goto(e.run_id ? `/runs/${e.run_id}` : e.app === 'billing' ? '/usage' : e.job_id ? `/${e.app}/${e.job_id}` : '/home');
	}

	function day(iso: string) {
		const d = new Date(iso);
		const today = new Date();
		const yesterday = new Date(Date.now() - 86400_000);
		if (d.toDateString() === today.toDateString()) return 'Today';
		if (d.toDateString() === yesterday.toDateString()) return 'Yesterday';
		return d.toLocaleDateString('en-US', { weekday: 'long', month: 'short', day: 'numeric' });
	}
	const groups = $derived.by(() => {
		const out: { day: string; items: EventRow[] }[] = [];
		for (const e of events) {
			const d = day(e.created_at);
			if (out.at(-1)?.day !== d) out.push({ day: d, items: [] });
			out.at(-1)!.items.push(e);
		}
		return out;
	});
	const icon = (e: EventRow) => (e.level === 'system' ? 'server' : appIcon(e.app));
</script>

<aside class="activity" class:full aria-label="Activity" data-testid="activity">
	{#if !full}
		<div class="bar">
			<div class="title"><span class="ic"><Icon name="activity" size={16} /></span><span class="name">Activity</span></div>
			<div class="acts">
				<button class="quiet" onclick={readAll}>Mark all read</button>
				<button class="x" onclick={() => (session.activityOpen = false)} aria-label="Close activity"><Icon name="x" size={16} /></button>
			</div>
		</div>
	{/if}
	<div class="tabs" role="tablist" aria-label="Show">
		<button role="tab" aria-selected={tab === 'all'} class:on={tab === 'all'} onclick={() => (tab = 'all')}>All</button>
		<button role="tab" aria-selected={tab === 'unread'} class:on={tab === 'unread'} onclick={() => (tab = 'unread')}
			>Unread{#if session.me?.unread}&nbsp;<span class="mono n">{session.me.unread}</span>{/if}</button
		>
		{#if full}
			<span class="grow"></span>
			<label class="sys"><input type="checkbox" bind:checked={showSystem} /> Show when computers wake and sleep</label>
			<button class="quiet" onclick={readAll}>Mark all read</button>
		{/if}
	</div>
	<div class="items">
		{#each groups as g (g.day)}
			<div class="day label">{g.day}</div>
			{#each g.items as e (e.id)}
				<button class="ev" class:unread={!e.read_at} onclick={() => open(e)}>
					<span class="dot" aria-label={e.read_at ? undefined : 'Unread'}></span>
					<span class="tile"><Icon name={icon(e)} size={15} stroke={2} /></span>
					<span class="what">
						<span class="line"><span class="t">{e.title}</span><span class="at mono">{clock(e.created_at)}</span></span>
						{#if e.body}<span class="s">{e.body.split('\n')[0]}</span>{/if}
						{#if e.actor_kind === 'assistant'}<span class="by">by the assistant</span>{/if}
					</span>
				</button>
			{/each}
		{:else}
			<div class="empty">{tab === 'unread' ? "You're all caught up." : 'Nothing has happened yet.'}</div>
		{/each}
	</div>
	{#if !full}
		<div class="foot">
			<a href="/settings/notifications" onclick={() => (session.activityOpen = false)}><Icon name="settings" size={13} />Notification settings</a>
			<a href="/activity" onclick={() => (session.activityOpen = false)}>See full history</a>
		</div>
	{/if}
</aside>

<style>
	.activity {
		width: 400px;
		flex-shrink: 0;
		height: calc(100vh - 52px - 12px);
		position: sticky;
		top: 52px;
		margin: 0 12px 12px 0;
		background: var(--surface);
		border-radius: 12px;
		border: 1px solid var(--line);
		display: flex;
		flex-direction: column;
		overflow: hidden;
		box-shadow: 0 12px 32px var(--shadow);
	}
	.full {
		width: auto;
		height: auto;
		position: static;
		margin: 0;
		border: 0;
		border-radius: 0;
		box-shadow: none;
		flex: 1;
		min-height: 0;
	}
	.bar {
		height: 52px;
		flex-shrink: 0;
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0 8px 0 14px;
		border-bottom: 1px solid var(--line);
	}
	.title {
		display: flex;
		align-items: center;
		gap: 10px;
	}
	.ic {
		width: 28px;
		height: 28px;
		border-radius: 8px;
		background: var(--raised);
		display: flex;
		align-items: center;
		justify-content: center;
	}
	.name {
		font-size: 15px;
		font-weight: 600;
	}
	.acts {
		display: flex;
		align-items: center;
		gap: 4px;
	}
	.quiet {
		height: 32px;
		padding: 0 10px;
		border-radius: 6px;
		border: 0;
		background: transparent;
		color: var(--mid);
		font: 500 13px Inter, system-ui, sans-serif;
		cursor: pointer;
	}
	.quiet:hover,
	.x:hover {
		color: var(--ink);
		background: var(--raised);
	}
	.x {
		width: 32px;
		height: 32px;
		border: 0;
		border-radius: 6px;
		background: transparent;
		color: var(--mid);
		display: flex;
		align-items: center;
		justify-content: center;
		cursor: pointer;
	}
	.tabs {
		display: flex;
		align-items: center;
		gap: 4px;
		padding: 10px 14px;
		border-bottom: 1px solid var(--line);
	}
	.tabs [role='tab'] {
		height: 28px;
		padding: 0 10px;
		border-radius: 6px;
		border: none;
		font: 500 13px Inter, system-ui, sans-serif;
		cursor: pointer;
		background: transparent;
		color: var(--mid);
	}
	.tabs [role='tab'].on {
		background: var(--ink);
		color: var(--bg);
	}
	.n {
		font-size: 11px;
	}
	.grow {
		flex: 1;
	}
	.sys {
		display: flex;
		align-items: center;
		gap: 6px;
		font-size: 13px;
		color: var(--mid);
		margin-right: 8px;
	}
	.items {
		flex: 1;
		overflow: auto;
		display: flex;
		flex-direction: column;
	}
	.day {
		padding: 14px 14px 8px;
	}
	.ev {
		display: flex;
		gap: 10px;
		padding: 12px 14px;
		border: 0;
		border-top: 1px solid var(--line);
		background: none;
		color: var(--ink);
		font: inherit;
		text-align: left;
		cursor: pointer;
	}
	.ev.unread {
		background: var(--pane);
	}
	.ev:hover {
		background: var(--raised);
	}
	.dot {
		width: 8px;
		height: 8px;
		border-radius: 9999px;
		flex-shrink: 0;
		margin-top: 4px;
	}
	.unread .dot {
		background: var(--accent);
	}
	.tile {
		width: 30px;
		height: 30px;
		border-radius: 8px;
		background: var(--raised);
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
	}
	.ev.unread .tile {
		background: var(--surface);
	}
	.what {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 2px;
	}
	.line {
		display: flex;
		justify-content: space-between;
		gap: 8px;
	}
	.t {
		font-size: 14px;
		font-weight: 500;
	}
	.unread .t {
		font-weight: 600;
	}
	.at {
		font-size: 11px;
		color: var(--low);
		flex-shrink: 0;
	}
	.s {
		font-size: 13px;
		color: var(--mid);
	}
	.by {
		font-size: 12px;
		color: var(--low);
	}
	.empty {
		padding: 40px 20px;
		text-align: center;
		color: var(--mid);
	}
	.foot {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 12px 14px;
		border-top: 1px solid var(--line);
		font-size: 13px;
	}
	.foot a {
		display: flex;
		align-items: center;
		gap: 6px;
		text-decoration: underline;
		text-underline-offset: 3px;
	}
	@media (max-width: 900px) {
		.activity:not(.full) {
			position: fixed;
			right: 0;
			top: 52px;
			z-index: 26;
			width: min(400px, calc(100vw - 12px));
		}
	}
</style>
