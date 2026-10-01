<script lang="ts">
	import type { Snippet } from 'svelte';
	import Icon from '$lib/ui/Icon.svelte';
	import Mark from '$lib/shell/Mark.svelte';

	// The frame for the steps after the email link: who's signed in, and Email › Phone › Plan.
	let { email, step, children, steps = true }: { email: string; step: 1 | 2 | 3; children: Snippet; steps?: boolean } = $props();
	const names = ['Email', 'Phone', 'Plan'];
</script>

<div class="frame">
	<header>
		<div class="brand"><Mark /><span>Croncave</span></div>
		<span class="who">Signed in as <span class="mono">{email}</span></span>
	</header>
	<main>
		{#if steps}
			<nav aria-label="Sign-up steps" class="steps">
				{#each names as n, i (n)}
					{#if i > 0}<span class="line"></span>{/if}
					{@const done = i < step}
					{@const here = i === step}
					<span class="step" class:here class:todo={!done && !here} aria-current={here ? 'step' : undefined}>
						<span class="num" class:done class:here class:todo={!done && !here}>
							{#if done}<Icon name="check" size={12} stroke={2.4} />{:else}{i + 1}{/if}
						</span>{n}
					</span>
				{/each}
			</nav>
		{/if}
		{@render children()}
	</main>
</div>

<style>
	.frame {
		min-height: 100vh;
		display: flex;
		flex-direction: column;
		background: var(--bg);
	}
	header {
		height: 60px;
		flex-shrink: 0;
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0 20px;
	}
	.brand {
		display: flex;
		align-items: center;
		gap: 10px;
		font-size: 15px;
		font-weight: 600;
	}
	.who {
		font-size: 13px;
		color: var(--mid);
	}
	.who .mono {
		color: var(--ink);
		font-size: 13px;
	}
	main {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 24px;
		padding: 0 16px 40px;
	}
	.steps {
		display: flex;
		align-items: center;
		gap: 12px;
	}
	.step {
		display: flex;
		align-items: center;
		gap: 8px;
		font-size: 13px;
		font-weight: 500;
	}
	.step.here {
		font-weight: 600;
	}
	.step.todo {
		color: var(--mid);
	}
	.num {
		width: 22px;
		height: 22px;
		border-radius: 9999px;
		display: flex;
		align-items: center;
		justify-content: center;
		font: 11px 'JetBrains Mono', ui-monospace, monospace;
	}
	.num.done {
		background: var(--accent-soft);
		color: var(--accent-ink);
	}
	.num.here {
		background: var(--accent);
		color: var(--on-accent);
	}
	.num.todo {
		border: 1px solid var(--line-strong);
		color: var(--mid);
	}
	.line {
		width: 40px;
		height: 1px;
		background: var(--line);
	}
	@media (max-width: 600px) {
		.who {
			display: none;
		}
	}
</style>
