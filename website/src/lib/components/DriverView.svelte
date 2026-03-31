<script>
	import { onMount, onDestroy } from 'svelte';
	import { browser } from '$app/environment';
	import truckLoaded from '$lib/assets/truck-loaded-tool-sprite.webp';
	import chekpointSprite from '$lib/assets/checkpoint-tool-sprite.webp';
	import CModal from './CModal.svelte';

	let { yak, oncomplete } = $props();

	const CHECKPOINT_COUNT = 7;
	const MAX_PLACEMENT_ATTEMPTS = 200;
	const LS_KEY = 'yak_mania_instructions_driver';

	let show_instructions = $state(false);

	let checkpoints = $state([]);
	let active_index = $state(0);
	let truck_x = $state(0);
	let truck_y = $state(0);
	let dragging = $state(false);
	let completed = $state(false);
	let truck_invisible = $state(false);

	let facing_left = $state(false);
	let last_x = 0;

	let game_area = $state(null);
	let completion_timeout = null;

	let collected = $derived(checkpoints.filter((c) => c.collected).length);

	let game_area_w = $derived(game_area?.clientWidth);
	let game_area_h = $derived(game_area?.clientHeight);
	let ref_vmin = $derived(
		Math.min(document.documentElement.clientWidth, document.documentElement.clientHeight)
	);
	let truck_elem_size = $derived(Math.min(ref_vmin * 0.25, 144));
	let checkpoint_elem_size = $derived(Math.min(ref_vmin * 0.18, 96));
	let hit_radius = $derived(checkpoint_elem_size * 0.4);
	let min_distance = $derived(checkpoint_elem_size * 1.5);

	function generate_checkpoints() {
		const points = [];
		const half_cp = checkpoint_elem_size / 2;
		const half_truck = truck_elem_size / 2;
		// Center exclusion: truck spawns at center, ban checkpoints from overlapping
		const cx = game_area_w / 2;
		const cy = game_area_h / 2;
		const excl_half_w = half_truck + half_cp + 4;
		const excl_half_h = half_truck + half_cp + 4;

		for (let i = 0; i < CHECKPOINT_COUNT; i++) {
			let x, y, valid;
			let attempts = 0;
			do {
				x = half_cp + Math.random() * (game_area_w - checkpoint_elem_size);
				y = half_cp + Math.random() * (game_area_h - checkpoint_elem_size);
				// Reject if inside truck spawn exclusion zone
				const in_excl =
					x > cx - excl_half_w &&
					x < cx + excl_half_w &&
					y > cy - excl_half_h &&
					y < cy + excl_half_h;
				// Reject if too close to an existing checkpoint
				const too_close = points.some((p) => {
					const dx = x - p.x;
					const dy = y - p.y;
					return Math.sqrt(dx * dx + dy * dy) < min_distance;
				});
				valid = !in_excl && !too_close;
				attempts++;
			} while (!valid && attempts < MAX_PLACEMENT_ATTEMPTS);
			points.push({ x, y, collected: false });
		}
		return points;
	}

	function reset_progress() {
		for (let i = 0; i < checkpoints.length; i++) {
			checkpoints[i].collected = false;
		}
		active_index = 0;
	}

	function check_hit() {
		if (completed || active_index >= checkpoints.length) return;
		// Check if the truck hit a wrong checkpoint first
		for (let i = active_index + 1; i < checkpoints.length; i++) {
			const wrong = checkpoints[i];
			const dx = truck_x - wrong.x;
			const dy = truck_y - wrong.y;
			if (Math.sqrt(dx * dx + dy * dy) < hit_radius) {
				reset_progress();
				return;
			}
		}
		const cp = checkpoints[active_index];
		const dx = truck_x - cp.x;
		const dy = truck_y - cp.y;
		if (Math.sqrt(dx * dx + dy * dy) < hit_radius) {
			checkpoints[active_index].collected = true;
			active_index++;

			if (active_index >= CHECKPOINT_COUNT) {
				completed = true;
				completion_timeout = setTimeout(() => {
					truck_invisible = true;
					oncomplete?.();
				}, 200);
			}
		}
	}

	function pointer_to_px(e) {
		if (!game_area) return null;
		const rect = game_area.getBoundingClientRect();
		return {
			x: Math.max(0, Math.min(game_area_w, e.clientX - rect.left)),
			y: Math.max(0, Math.min(game_area_h, e.clientY - rect.top))
		};
	}

	function handle_pointer_down(e) {
		if (completed) return;
		dragging = true;
		e.currentTarget.setPointerCapture(e.pointerId);
	}

	function handle_pointer_move(e) {
		if (!dragging || completed) return;
		const pos = pointer_to_px(e);
		if (!pos) return;
		if (pos.x < last_x) facing_left = true;
		else if (pos.x > last_x) facing_left = false;
		last_x = pos.x;
		truck_x = pos.x;
		truck_y = pos.y;
		check_hit();
	}

	function handle_pointer_up() {
		dragging = false;
	}

	onMount(() => {
		truck_x = game_area_w / 2;
		truck_y = game_area_h / 2;
		last_x = truck_x;
		checkpoints = generate_checkpoints();
		if (browser && !localStorage.getItem(LS_KEY)) {
			show_instructions = true;
		}
	});

	function dismiss_instructions() {
		show_instructions = false;
		if (browser) localStorage.setItem(LS_KEY, '1');
	}

	onDestroy(() => {
		if (completion_timeout) clearTimeout(completion_timeout);
	});
</script>

<div class="relative flex flex-1 flex-col overflow-hidden">
	<div class="flex items-center gap-4 px-2 pt-2">
		<div class="flex-1 text-center">
			<p class="text-sm font-medium">
				Drag your truck through the checkpoints <b>in order</b>!
			</p>
		</div>
		<div class="flex shrink flex-col items-center gap-1">
			<span
				class="flex items-center gap-2 rounded-full bg-base-200 px-4 py-1 text-xl font-semibold"
			>
				<img src={chekpointSprite} alt="Checkpoint" class="h-6 w-6 object-contain" />
				{collected} / {CHECKPOINT_COUNT}
			</span>
			<p class="text-sm opacity-50">Yak {yak.id.slice(0, 8)}</p>
		</div>
	</div>

	<div class="game-area relative flex flex-1" bind:this={game_area}>
		{#each checkpoints as cp, i (i)}
			<div
				class="absolute flex -translate-x-1/2 -translate-y-1/2 items-center justify-center"
				style="top: {cp.y}px; left: {cp.x}px"
			>
				<img
					src={chekpointSprite}
					alt="Checkpoint {i + 1}"
					class="h-[18vmin] max-h-24 w-[18vmin] max-w-24 object-contain transition-opacity"
					class:opacity-100={!cp.collected}
					class:opacity-30={cp.collected}
					class:-scale-x-100={cp.x > game_area_w / 2}
					style={i === active_index && !cp.collected
						? 'animation: checkpoint-glow 1.2s ease-in-out infinite'
						: ''}
					draggable="false"
				/>
				<span
					class="absolute text-3xl font-bold text-white text-shadow-black text-shadow-lg"
					class:animate-bounce={i === active_index && !cp.collected}
				>
					{#if cp.collected}
						✓
					{:else}
						{i + 1}
					{/if}
				</span>
			</div>
		{/each}

		<img
			src={truckLoaded}
			alt="Truck"
			class="absolute h-[25vmin] max-h-36 w-[25vmin] max-w-36 -translate-x-1/2 -translate-y-1/2 cursor-grab object-contain active:cursor-grabbing"
			draggable="false"
			class:invisible={truck_invisible}
			class:-scale-x-100={facing_left}
			style="top: {truck_y}px; left: {truck_x}px"
			onpointerdown={handle_pointer_down}
			onpointermove={handle_pointer_move}
			onpointerup={handle_pointer_up}
			onpointercancel={handle_pointer_up}
		/>
	</div>
</div>

<CModal bind:open={show_instructions} id="driver_instructions" onclose={dismiss_instructions}>
	<h3 class="text-lg font-bold">Driver</h3>
	<p class="py-4">
		Hit the road! <b>Drag your truck</b> through the numbered checkpoints <b>in order</b>. If you
		hit a wrong checkpoint, all progress is reset! Follow the glowing marker to find the next one.
	</p>
	<div class="modal-action">
		<button class="btn btn-primary" onclick={dismiss_instructions}>Understood</button>
	</div>
</CModal>
