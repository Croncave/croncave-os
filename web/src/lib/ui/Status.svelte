<script lang="ts">
	import { statusOf } from '$lib/format';
	// A status always pairs its color with a word. Pills by default, as in the designs;
	// `plain` drops the fill for dense rows.
	let { status, word, plain = false }: { status: string; word?: string; plain?: boolean } = $props();
	const s = $derived(statusOf(status));
</script>

<span class="status" class:plain style="--tone: var(--{s.tone}); --soft: var(--{s.tone}-soft)" data-status={status}>
	<span class="dot" class:pulse={s.tone === 'working'}></span>{word ?? s.word}
</span>

<style>
	.status {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		height: 22px;
		padding: 0 8px;
		border-radius: 4px;
		background: var(--soft);
		color: var(--tone);
		font-size: 12px;
		font-weight: 600;
		white-space: nowrap;
		flex-shrink: 0;
	}
	.plain {
		background: none;
		padding: 0;
		height: auto;
		font-size: 13px;
		font-weight: 500;
	}
	.dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: var(--tone);
	}
	.pulse {
		animation: p 1.4s ease-in-out infinite;
	}
	@keyframes p {
		50% {
			opacity: 0.35;
		}
	}
</style>
