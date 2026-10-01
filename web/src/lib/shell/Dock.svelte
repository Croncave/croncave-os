<script lang="ts">
	import { page } from '$app/state';
	import { session } from '$lib/session.svelte';

	const apps = [
		{ href: '/home', label: 'Home', icon: '⌂' },
		{ href: '/files', label: 'Files', icon: '▤' },
		{ href: '/activity', label: 'Activity', icon: '◷' }
	];
	const installed = [
		{ href: '/scripts', label: 'Scripts', icon: '❯_' },
		{ href: '/watcher', label: 'Watcher', icon: '◉' },
		{ href: '/code', label: 'Code', icon: '{}' }
	];
	const on = (href: string) => page.url.pathname === href || page.url.pathname.startsWith(href + '/');
</script>

<nav class="dock" aria-label="Apps">
	{#each apps as a (a.href)}
		{#if a.href === '/activity'}
			<button class="app" class:on={session.activityOpen || on(a.href)} onclick={() => (session.activityOpen = !session.activityOpen)}><span class="ic">{a.icon}</span>{a.label}</button>
		{:else}
			<a class="app" class:on={on(a.href)} href={a.href}><span class="ic">{a.icon}</span>{a.label}</a>
		{/if}
	{/each}
	<div class="sep"></div>
	{#each installed as a (a.href)}
		<a class="app" class:on={on(a.href)} href={a.href}><span class="ic mono">{a.icon}</span>{a.label}</a>
	{/each}
	<div class="sep"></div>
	<a class="app" class:on={on('/apps')} href="/apps"><span class="ic">⊞</span>All apps</a>
	<a class="app" href="/apps#get"><span class="ic">＋</span>Get apps</a>
	<span class="spacer"></span>
	<a class="app" class:on={on('/computer')} href="/computer"><span class="ic">⚙</span>Settings</a>
</nav>

<style>
	.dock {
		width: 84px;
		border-right: 1px solid var(--line);
		background: var(--pane);
		display: flex;
		flex-direction: column;
		padding: 10px 6px;
		gap: 2px;
	}
	.app {
		display: grid;
		justify-items: center;
		gap: 2px;
		font-size: 11.5px;
		color: var(--mid);
		padding: 8px 4px;
		border-radius: 10px;
		background: none;
		border: 0;
		font-family: inherit;
		cursor: pointer;
	}
	.app:hover {
		background: var(--surface);
		text-decoration: none;
		color: var(--ink);
	}
	.on {
		background: var(--surface);
		color: var(--ink);
		box-shadow: inset 2px 0 0 var(--accent);
	}
	.ic {
		font-size: 17px;
		line-height: 1.2;
	}
	.sep {
		height: 1px;
		background: var(--line);
		margin: 6px 8px;
	}
	@media (max-width: 760px) {
		.dock {
			width: 64px;
		}
	}
</style>
