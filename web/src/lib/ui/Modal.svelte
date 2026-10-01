<script lang="ts">
	import type { Snippet } from 'svelte';
	import Icon from './Icon.svelte';
	let {
		title,
		eyebrow,
		lede,
		onclose,
		children,
		wide = false,
		closable = true
	}: { title: string; eyebrow?: string; lede?: string; onclose: () => void; children: Snippet; wide?: boolean; closable?: boolean } = $props();
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && closable && onclose()} />
<div class="backdrop" role="presentation" onclick={(e) => closable && e.target === e.currentTarget && onclose()}>
	<div class="modal" class:wide role="dialog" aria-modal="true" aria-label={title}>
		<div class="head">
			{#if eyebrow}<span class="eyebrow">{eyebrow}</span>{/if}
			<h2>{title}</h2>
			{#if lede}<p class="lede">{lede}</p>{/if}
			{#if closable}<button class="x" onclick={onclose} aria-label="Close"><Icon name="x" size={16} /></button>{/if}
		</div>
		{@render children()}
	</div>
</div>

<style>
	.backdrop {
		position: fixed;
		inset: 0;
		background: var(--overlay);
		display: grid;
		place-items: center;
		padding: 6vh 16px;
		z-index: 50;
		overflow: auto;
	}
	.modal {
		width: min(560px, 100%);
		background: var(--surface);
		border: 1px solid var(--line);
		border-radius: 16px;
		box-shadow: 0 24px 60px var(--shadow);
		padding: 28px;
		display: flex;
		flex-direction: column;
		gap: 18px;
	}
	.wide {
		width: min(860px, 100%);
	}
	.head {
		position: relative;
		display: flex;
		flex-direction: column;
		gap: 6px;
		padding-right: 28px;
	}
	.eyebrow {
		font-size: 13px;
		font-weight: 600;
		color: var(--accent-ink);
	}
	h2 {
		font-size: 22px;
		line-height: 30px;
		font-weight: 600;
	}
	.lede {
		font-size: 14px;
		line-height: 21px;
		color: var(--mid);
	}
	.x {
		position: absolute;
		top: -6px;
		right: -8px;
		width: 30px;
		height: 30px;
		border-radius: 6px;
		background: none;
		border: 0;
		color: var(--mid);
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
	}
	.x:hover {
		background: var(--raised);
		color: var(--ink);
	}
</style>
