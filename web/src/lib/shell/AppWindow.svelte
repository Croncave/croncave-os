<script lang="ts">
	import type { Snippet } from 'svelte';
	import Icon from '$lib/ui/Icon.svelte';

	// The framed window every app opens in: an icon and title (with an optional trail of
	// places), actions on the right, and the app's own body below.
	let {
		icon,
		title,
		href,
		crumbs = [],
		actions,
		children
	}: {
		icon: string;
		title: string;
		href?: string;
		crumbs?: { label: string; href?: string }[];
		actions?: Snippet;
		children: Snippet;
	} = $props();
</script>

<section class="window" aria-label={title}>
	<header class="bar">
		<div class="title">
			<span class="ic"><Icon name={icon} size={16} /></span>
			{#if href && crumbs.length}<a class="name" {href}>{title}</a>{:else}<span class="name">{title}</span>{/if}
			{#each crumbs as c, i (i)}
				<span class="sep">/</span>
				{#if c.href}<a class="crumb" href={c.href}>{c.label}</a>{:else}<span class="crumb">{c.label}</span>{/if}
			{/each}
		</div>
		<div class="actions">{@render actions?.()}</div>
	</header>
	<div class="body">{@render children()}</div>
</section>

<style>
	.window {
		height: calc(100vh - 52px - 12px);
		margin: 0 12px 12px 0;
		background: var(--surface);
		border: 1px solid var(--line);
		border-radius: 12px;
		display: flex;
		flex-direction: column;
		overflow: hidden;
		min-width: 0;
	}
	.bar {
		height: 52px;
		flex-shrink: 0;
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		padding: 0 12px 0 14px;
		border-bottom: 1px solid var(--line);
	}
	.title {
		display: flex;
		align-items: center;
		gap: 10px;
		min-width: 0;
		white-space: nowrap;
	}
	.ic {
		width: 28px;
		height: 28px;
		border-radius: 8px;
		background: var(--raised);
		color: var(--ink);
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
	}
	.name {
		font-size: 15px;
		font-weight: 600;
		color: var(--ink);
	}
	.sep {
		color: var(--line-strong);
	}
	.crumb {
		font-size: 14px;
		color: var(--mid);
		overflow: hidden;
		text-overflow: ellipsis;
	}
	a.crumb:hover,
	a.name:hover {
		color: var(--ink);
		text-decoration: none;
	}
	.actions {
		display: flex;
		align-items: center;
		gap: 8px;
	}
	.body {
		flex: 1;
		min-height: 0;
		display: flex;
		overflow: hidden;
	}
	@media (max-width: 760px) {
		.window {
			height: auto;
			min-height: calc(100vh - 52px - 12px);
			margin: 0 8px 8px 0;
		}
		.body {
			flex-direction: column;
			overflow: visible;
		}
	}
</style>
