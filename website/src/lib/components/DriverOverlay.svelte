<script>
	import { onMount } from 'svelte';
	import truckEmpty from '$lib/assets/truck-empty-tool-sprite.webp';
	import truckLoaded from '$lib/assets/truck-loaded-tool-sprite.webp';

	let { direction, waiting = false, onfinished } = $props();

	const CROSS_DURATION = 1000;
	const PAUSE_DURATION = 200;

	let phase = $state(1);
	let animation_done = $state(false);

	// Buy (direction 'in'):  phase 1 = empty enters from right, phase 3 = loaded exits to right
	// Sell (direction 'out'): phase 1 = loaded enters from left, phase 3 = empty exits to left
	let phase1_sprite = $derived(direction === 'in' ? truckEmpty : truckLoaded);
	let phase3_sprite = $derived(direction === 'in' ? truckLoaded : truckEmpty);
	let resting_translation = $derived(
		direction === 'in' ? '-translate-x-[20vw]' : 'translate-x-[20vw]'
	);
	let phase1_animation = $derived(
		direction === 'in' ? 'truck-enter-from-right' : 'truck-enter-from-left'
	);
	let phase3_animation = $derived(
		direction === 'in' ? 'truck-exit-to-right' : 'truck-exit-to-left'
	);

	function handle_phase1_end() {
		phase = 2;
		setTimeout(() => {
			phase = 3;
		}, PAUSE_DURATION);
	}

	function handle_phase3_end() {
		animation_done = true;
		onfinished?.();
	}

	onMount(() => {
		// Safety timeout in case animationend doesn't fire
		const fallback = setTimeout(
			() => {
				if (!animation_done) {
					animation_done = true;
					onfinished?.();
				}
			},
			CROSS_DURATION * 2 + PAUSE_DURATION + 200
		);
		return () => clearTimeout(fallback);
	});
</script>

<div class="absolute inset-0 z-10 flex flex-col items-center justify-center bg-base-100/70">
	{#if phase === 1}
		<img
			src={phase1_sprite}
			alt="Truck"
			class="h-32 object-contain"
			style="animation: {phase1_animation} {CROSS_DURATION}ms ease-out forwards"
			onanimationend={handle_phase1_end}
		/>
	{:else if phase === 2}
		<img
			src={phase1_sprite}
			alt="Truck"
			class="h-32 {resting_translation} -scale-x-100 object-contain"
		/>
	{:else if phase === 3}
		<img
			src={phase3_sprite}
			alt="Truck"
			class="h-32 object-contain"
			style="animation: {phase3_animation} {CROSS_DURATION}ms ease-in forwards"
			onanimationend={handle_phase3_end}
		/>
	{/if}

	{#if animation_done && waiting}
		<div class="mt-6 flex flex-col items-center gap-2">
			<span class="loading loading-md loading-spinner"></span>
			<p class="text-sm opacity-60">Please wait&hellip;</p>
		</div>
	{/if}
</div>
