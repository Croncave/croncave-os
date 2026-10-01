<script lang="ts">
	import { toasts } from './toast.svelte';
</script>

<div class="toasts" aria-live="polite">
	{#each toasts.items as t (t.id)}
		<div class="toast" class:bad={t.bad}>{t.text}</div>
	{/each}
</div>

<style>
	.toasts {
		position: fixed;
		bottom: 20px;
		left: 50%;
		transform: translateX(-50%);
		display: grid;
		gap: 8px;
		z-index: 60;
	}
	.toast {
		background: var(--surface);
		border: 1px solid var(--line);
		border-left: 3px solid var(--live);
		padding: 10px 14px;
		border-radius: 10px;
		max-width: 480px;
	}
	.bad {
		border-left-color: var(--failed);
	}
</style>
