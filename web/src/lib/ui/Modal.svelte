<script lang="ts">
	import type { Snippet } from 'svelte';
	let { title, onclose, children, wide = false }: { title: string; onclose: () => void; children: Snippet; wide?: boolean } = $props();
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onclose()} />
<div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose()}>
	<div class="modal card" class:wide role="dialog" aria-modal="true" aria-label={title}>
		<div class="row head"><h2>{title}</h2><span class="spacer"></span><button class="x" onclick={onclose} aria-label="Close">✕</button></div>
		{@render children()}
	</div>
</div>

<style>
	.backdrop {
		position: fixed;
		inset: 0;
		background: var(--overlay);
		display: grid;
		place-items: start center;
		padding: 8vh 16px;
		z-index: 50;
		overflow: auto;
	}
	.modal {
		width: min(520px, 100%);
		display: grid;
		gap: 14px;
	}
	.wide {
		width: min(860px, 100%);
	}
	.x {
		background: none;
		border: 0;
		color: var(--mid);
		cursor: pointer;
		font-size: 16px;
	}
</style>
