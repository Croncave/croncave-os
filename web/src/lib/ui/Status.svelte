<script lang="ts">
	import { statusOf } from '$lib/format';
	// A status always pairs its color with a word.
	let { status, word }: { status: string; word?: string } = $props();
	const s = $derived(statusOf(status));
</script>

<span class="status" style="--tone: var(--{s.tone})" data-status={status}>
	<span class="dot" class:pulse={s.tone === 'working'}></span>{word ?? s.word}
</span>

<style>
	.status {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		color: var(--tone);
		font-size: 13px;
		font-weight: 500;
		white-space: nowrap;
	}
	.dot {
		width: 8px;
		height: 8px;
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
