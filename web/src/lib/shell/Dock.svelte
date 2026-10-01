<script lang="ts">
	import { page } from '$app/state';
	import { session } from '$lib/session.svelte';
	import Icon from '$lib/ui/Icon.svelte';

	// Apps on this computer, then ones that aren't built yet (shown, but not openable).
	const apps = [
		{ href: '/watcher', label: 'Watcher', icon: 'eye' },
		{ label: 'Data', icon: 'data', soon: true },
		{ href: '/scripts', label: 'Scripts', icon: 'code' },
		{ label: 'Fetcher', icon: 'download', soon: true },
		{ href: '/code', label: 'Code', icon: 'braces' }
	];
	const on = (href: string) => page.url.pathname === href || page.url.pathname.startsWith(href + '/');
	const unread = $derived(session.me?.unread ?? 0);
</script>

{#snippet tile(icon: string, label: string, current: boolean, badge = 0)}
	<span class="tile" class:current>
		<Icon name={icon} size={20} stroke={2} />
		{#if badge > 0}<span class="badge" data-testid="unread" aria-label="{badge} new">{badge > 99 ? '99+' : badge}</span>{/if}
	</span>
	<span class="lbl" class:current>{label}</span>
{/snippet}

<nav class="dock" aria-label="Apps">
	<a class="app" href="/home" aria-current={on('/home') ? 'page' : undefined}>{@render tile('home', 'Home', on('/home'))}</a>
	<a class="app" href="/files" aria-current={on('/files') ? 'page' : undefined}>{@render tile('folder', 'Files', on('/files'))}</a>
	<button class="app" onclick={() => (session.activityOpen = !session.activityOpen)} aria-pressed={session.activityOpen} data-testid="bell">
		{@render tile('activity', 'Activity', session.activityOpen || on('/activity'), unread)}
	</button>
	<span class="sep"></span>
	{#each apps as a (a.label)}
		{#if a.soon}
			<span class="app soon" title="{a.label} is coming soon" aria-disabled="true">{@render tile(a.icon, a.label, false)}<span class="soon-tag">Soon</span></span>
		{:else}
			<a class="app" href={a.href} aria-current={on(a.href!) ? 'page' : undefined}>{@render tile(a.icon, a.label, on(a.href!))}</a>
		{/if}
	{/each}
	<span class="sep"></span>
	<a class="app" href="/apps" aria-current={on('/apps') ? 'page' : undefined}>{@render tile('apps', 'All apps', on('/apps'))}</a>
	<a class="app" href="/apps#get">
		<span class="tile get"><Icon name="plus" size={18} /></span><span class="lbl">Get apps</span>
	</a>
	<span class="grow"></span>
	<a class="app" href="/settings" aria-current={on('/settings') || on('/computer') ? 'page' : undefined}
		>{@render tile('settings', 'Settings', on('/settings') || on('/computer') || on('/usage') || on('/plans'))}</a
	>
</nav>

<style>
	.dock {
		width: 80px;
		flex-shrink: 0;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 14px;
		padding: 8px 0 16px;
		height: calc(100vh - 52px);
		position: sticky;
		top: 52px;
		overflow-y: auto;
		scrollbar-width: none;
	}
	.app {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 5px;
		width: 64px;
		background: none;
		border: 0;
		padding: 0;
		font: inherit;
		cursor: pointer;
		position: relative;
	}
	.app:hover {
		text-decoration: none;
	}
	.tile {
		position: relative;
		width: 42px;
		height: 42px;
		border-radius: 11px;
		background: var(--raised);
		color: var(--ink);
		display: flex;
		align-items: center;
		justify-content: center;
	}
	.app:hover .tile:not(.current) {
		background: var(--track);
	}
	.tile.current {
		background: var(--accent);
		color: var(--on-accent);
	}
	.get {
		width: 40px;
		height: 40px;
		background: none;
		border: 1.5px dashed var(--line-strong);
		color: var(--mid);
	}
	.app:hover .get {
		background: none;
		color: var(--ink);
	}
	.lbl {
		font-size: 11px;
		font-weight: 500;
		color: var(--mid);
		white-space: nowrap;
	}
	.lbl.current {
		font-weight: 600;
		color: var(--ink);
	}
	.badge {
		position: absolute;
		top: -6px;
		right: -8px;
		min-width: 18px;
		height: 18px;
		padding: 0 5px;
		border-radius: 9999px;
		background: var(--needs);
		color: var(--on-accent);
		border: 2px solid var(--bg);
		font: 600 10px 'JetBrains Mono', ui-monospace, monospace;
		display: flex;
		align-items: center;
		justify-content: center;
	}
	.sep {
		width: 28px;
		height: 1px;
		background: var(--line);
		flex-shrink: 0;
	}
	.grow {
		flex: 1;
	}
	.soon {
		cursor: default;
	}
	.soon .tile {
		opacity: 0.45;
	}
	.soon .lbl {
		opacity: 0.6;
	}
	.soon-tag {
		position: absolute;
		top: -5px;
		right: 0;
		font: 600 9px 'JetBrains Mono', ui-monospace, monospace;
		letter-spacing: 0.04em;
		text-transform: uppercase;
		padding: 1px 4px;
		border-radius: 4px;
		background: var(--raised);
		color: var(--mid);
		border: 1px solid var(--line);
	}
	@media (max-width: 760px) {
		.dock {
			width: 64px;
			gap: 10px;
		}
		.app {
			width: 56px;
		}
	}
</style>
