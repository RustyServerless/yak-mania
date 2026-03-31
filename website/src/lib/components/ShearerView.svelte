<script>
	import { onMount, onDestroy } from 'svelte';
	import { browser } from '$app/environment';
	import hairyYak from '$lib/assets/hairy-yak-sprite.webp';
	import nakedYak from '$lib/assets/naked-yak-sprite.webp';
	import trimmerSprite from '$lib/assets/trimmer-tool-sprite.webp';
	import wheelSprite from '$lib/assets/wheel-tool-sprite.webp';
	import CModal from './CModal.svelte';

	let { yak, oncomplete } = $props();

	// --- Constants ---
	const ROTATIONS_REQUIRED = 10;
	const MIN_OPACITY = 0.15;
	const SHEARING_DURATION = 1000; // ms for trimmer to fly across
	const LS_KEY = 'yak_mania_instructions_shearer';

	// --- Instruction modal ---
	let show_instructions = $state(false);

	// --- Spin hint arrow ---
	let has_started_spinning = $state(false);

	// --- Charging state ---
	let cumulative_angle = $state(0);
	let wheel_rotation = $state(0);
	let dragging = $state(false);
	let last_angle = null;

	// --- Phases: 'charging' | 'shearing' | 'done' ---
	let phase = $state('charging');
	let yak_invisible = $state(false);

	let wheel_elem = $state(null);
	let done_timeout = null;

	// Charge from 0 to 1
	let charge = $derived(Math.min(1, cumulative_angle / (ROTATIONS_REQUIRED * 2 * Math.PI)));
	let rotations_done = $derived(Math.floor(cumulative_angle / (2 * Math.PI)));

	// Trimmer reveal: clip-path reveals left-to-right based on charge
	let trimmer_clip = $derived(`inset(0 ${(1 - charge) * 100}% 0 0)`);

	// --- Spin tracking ---
	function get_angle(e) {
		if (!wheel_elem) return 0;
		const rect = wheel_elem.getBoundingClientRect();
		const cx = rect.left + rect.width / 2;
		const cy = rect.top + rect.height / 2;
		return Math.atan2(e.clientY - cy, e.clientX - cx);
	}

	function handle_pointer_down(e) {
		if (phase !== 'charging') return;
		dragging = true;
		has_started_spinning = true;
		last_angle = get_angle(e);
		e.currentTarget.setPointerCapture(e.pointerId);
	}

	function handle_pointer_move(e) {
		if (!dragging || phase !== 'charging') return;
		const current_angle = get_angle(e);
		if (last_angle === null) {
			last_angle = current_angle;
			return;
		}

		let delta = current_angle - last_angle;

		// Normalize delta to [-PI, PI] to handle wrapping around ±PI
		if (delta > Math.PI) delta -= 2 * Math.PI;
		if (delta < -Math.PI) delta += 2 * Math.PI;

		// Only count clockwise rotation (positive delta in screen coordinates)
		cumulative_angle += Math.max(0, delta);
		wheel_rotation += (delta * 180) / Math.PI;
		last_angle = current_angle;

		// Check if fully charged
		if (charge >= 1) {
			dragging = false;
			last_angle = null;
			phase = 'shearing';
		}
	}

	function handle_pointer_up() {
		dragging = false;
		last_angle = null;
	}

	function handle_shearing_end() {
		phase = 'done';
		done_timeout = setTimeout(() => {
			yak_invisible = true;
			oncomplete?.();
		}, 600);
	}

	onMount(() => {
		if (browser && !localStorage.getItem(LS_KEY)) {
			show_instructions = true;
		}
	});

	function dismiss_instructions() {
		show_instructions = false;
		if (browser) localStorage.setItem(LS_KEY, '1');
	}

	onDestroy(() => {
		if (done_timeout) clearTimeout(done_timeout);
	});
</script>

<div class="relative flex flex-1 flex-col overflow-hidden">
	<!-- Progress counter -->
	<div class="flex items-center gap-4 px-2 pt-2">
		<div class="flex-1 text-center">
			<p class="text-sm font-medium">Spin the wheel to power the trimmer!</p>
		</div>
		<div class="flex shrink flex-col items-center gap-1">
			<span
				class="flex items-center gap-2 rounded-full bg-base-200 px-4 py-1 text-xl font-semibold"
			>
				<img src={trimmerSprite} alt="Trimmer" class="h-6 w-6 object-contain" />
				{rotations_done} / {ROTATIONS_REQUIRED}
			</span>
			<p class="text-sm opacity-50">Yak {yak.id.slice(0, 8)}</p>
		</div>
	</div>

	<!-- Game area -->
	<div class="game-area relative flex flex-1 flex-col items-center justify-between">
		<div class="relative flex w-full max-w-xl justify-between px-4">
			<!-- Yak sprite area -->
			<div class="relative h-[40vmin] max-h-40 w-[40vmin] max-w-40 justify-self-start">
				<img
					src={hairyYak}
					alt="Hairy Yak"
					class="absolute inset-0 h-full w-full object-contain transition-opacity delay-500 duration-500"
					class:opacity-0={phase === 'done' || phase === 'shearing'}
					draggable="false"
				/>
				<img
					src={nakedYak}
					alt="Naked Yak"
					class="absolute inset-0 h-full w-full object-contain transition-opacity delay-500 duration-500"
					class:opacity-0={!(phase === 'done' || phase === 'shearing')}
					class:invisible={yak_invisible}
					draggable="false"
				/>
			</div>
			<!-- Trimmer charge indicator (top-right) -->
			<div
				class="relative h-[40vmin] max-h-40 w-[40vmin] max-w-40 place-self-end"
				class:invisible={phase !== 'charging'}
			>
				<!-- Background layer: low opacity trimmer silhouette -->
				<img
					src={trimmerSprite}
					alt="Trimmer (uncharged)"
					class="absolute inset-0 h-full w-full object-contain"
					style="opacity: {MIN_OPACITY}"
					draggable="false"
				/>
				<!-- Foreground layer: revealed left-to-right -->
				<img
					src={trimmerSprite}
					alt="Trimmer (charged)"
					class="absolute inset-0 h-full w-full object-contain"
					style="clip-path: {trimmer_clip}"
					draggable="false"
				/>
			</div>

			<!-- Trimmer flying across during shearing phase -->
			{#if phase === 'shearing'}
				<img
					src={trimmerSprite}
					alt="Trimmer"
					class="pointer-events-none absolute h-[40vmin] max-h-40 w-[40vmin] max-w-40 object-contain"
					style="animation: trimmer-fly {SHEARING_DURATION}ms ease-in-out forwards"
					onanimationend={handle_shearing_end}
					draggable="false"
				/>
			{/if}
		</div>

		<!-- Wheel spin zone -->
		{#if phase === 'charging'}
			<div class="relative flex items-center justify-center">
				<div
					bind:this={wheel_elem}
					class="flex h-[60vmin] max-h-60 w-[60vmin] max-w-60 cursor-grab items-center justify-center
						rounded-full border-4 border-dashed border-base-300
						active:cursor-grabbing"
					role="button"
					tabindex="0"
					onpointerdown={handle_pointer_down}
					onpointermove={handle_pointer_move}
					onpointerup={handle_pointer_up}
					onpointercancel={handle_pointer_up}
				>
					<img
						src={wheelSprite}
						alt="Spin wheel"
						class="pointer-events-none h-11/12 w-11/12 object-contain"
						style="transform: rotate({wheel_rotation}deg)"
						draggable="false"
					/>
				</div>

				<!-- Curved arrow hint: clockwise rotation indicator -->
				{#if !has_started_spinning}
					<svg
						class="pointer-events-none absolute h-[70vmin] max-h-70 w-[70vmin] max-w-70 animate-[spin-hint-arrow_1.5s_ease-in-out_infinite] overflow-visible text-base-content"
						viewBox="0 0 100 100"
					>
						<defs>
							<marker
								id="spin-arrow"
								markerWidth="6"
								markerHeight="5"
								refX="2"
								refY="2.5"
								orient="auto"
							>
								<polygon points="0 0, 6 2.5, 0 5" class="fill-current" />
							</marker>
						</defs>
						<path
							d="M 50 4 A 46 46 0 0 1 96 50"
							class="fill-none stroke-current stroke-3"
							stroke-linecap="round"
							marker-end="url(#spin-arrow)"
							opacity="0.7"
						/>
					</svg>
				{/if}
			</div>
		{/if}
	</div>
</div>

<CModal bind:open={show_instructions} id="shearer_instructions" onclose={dismiss_instructions}>
	<h3 class="text-lg font-bold">Shearer</h3>
	<p class="py-4">
		Time for a trim! Spin the wheel <b>clockwise</b> to charge the trimmer. You need
		{ROTATIONS_REQUIRED} full rotations to power it up!
	</p>
	<div class="modal-action">
		<button class="btn btn-primary" onclick={dismiss_instructions}>Understood</button>
	</div>
</CModal>
