<script>
	let { sprite, direction, waiting = false, onfinished } = $props();

	let animation_done = $state(false);

	function handle_animation_end() {
		animation_done = true;
		onfinished?.();
	}

	let animation_name = $derived(direction === 'in' ? 'yak-zoom-in' : 'yak-zoom-out');
</script>

<div class="absolute inset-0 z-10 flex flex-col items-center justify-center bg-base-100/70">
	<img
		src={sprite}
		alt="Yak"
		class="h-[40vmin] max-h-48 w-[40vmin] max-w-48 object-contain transition-all"
		style="animation: {animation_name} 1.5s linear forwards"
		onanimationend={handle_animation_end}
	/>

	{#if animation_done && waiting}
		<div class="mt-6 flex flex-col items-center gap-2">
			<span class="loading loading-md loading-spinner"></span>
			<p class="text-sm opacity-60">Please wait&hellip;</p>
		</div>
	{/if}
</div>
