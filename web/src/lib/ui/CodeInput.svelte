<script lang="ts">
	// A one-time code as six boxes. Typing moves along, backspace moves back, and pasting
	// (or a phone's autofill into the first box) fills them all.
	let { value = $bindable(''), length = 6, oncomplete }: { value: string; length?: number; oncomplete?: () => void } = $props();
	let boxes: HTMLInputElement[] = [];
	const digits = $derived(Array.from({ length }, (_, i) => value[i] ?? ''));

	function set(i: number, raw: string) {
		const d = raw.replace(/\D/g, '');
		if (d.length > 1) {
			// Pasted or autofilled: spread from this box on.
			value = (value.slice(0, i) + d).slice(0, length);
			boxes[Math.min(value.length, length - 1)]?.focus();
		} else {
			const arr = digits.slice();
			arr[i] = d;
			value = arr.join('').slice(0, length);
			if (d && i < length - 1) boxes[i + 1]?.focus();
		}
		if (value.length === length) oncomplete?.();
	}
	function key(i: number, e: KeyboardEvent) {
		if (e.key === 'Backspace' && !digits[i] && i > 0) {
			e.preventDefault();
			value = value.slice(0, i - 1);
			boxes[i - 1]?.focus();
		} else if (e.key === 'ArrowLeft' && i > 0) boxes[i - 1]?.focus();
		else if (e.key === 'ArrowRight' && i < length - 1) boxes[i + 1]?.focus();
	}
</script>

<div class="code" role="group" aria-label="Six-digit code">
	{#each digits as d, i (i)}
		<input
			bind:this={boxes[i]}
			value={d}
			inputmode="numeric"
			autocomplete={i === 0 ? 'one-time-code' : 'off'}
			aria-label={i === 0 ? 'Code' : `Digit ${i + 1}`}
			oninput={(e) => set(i, e.currentTarget.value)}
			onkeydown={(e) => key(i, e)}
			onfocus={(e) => e.currentTarget.select()}
		/>
	{/each}
</div>

<style>
	.code {
		display: flex;
		gap: 8px;
	}
	input {
		width: 48px;
		height: 56px;
		padding: 0;
		text-align: center;
		border-radius: 8px;
		background: var(--pane);
		font: 500 22px 'JetBrains Mono', ui-monospace, monospace;
	}
	input:focus,
	input:focus-visible {
		border: 2px solid var(--working);
		box-shadow: none;
		outline: none;
	}
</style>
