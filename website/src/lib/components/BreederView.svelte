<script>
	import { onMount, onDestroy } from 'svelte';
	import { browser } from '$app/environment';
	import babyYak from '$lib/assets/baby-yak-sprite.webp';
	import youngYak from '$lib/assets/young-yak-sprite.webp';
	import hairyYak from '$lib/assets/hairy-yak-sprite.webp';
	import haySprite from '$lib/assets/hay-tool-sprite.webp';
	import CModal from './CModal.svelte';

	let { yak, oncomplete } = $props();

	const HAYSTACKS_TO_COLLECT = 8;
	const HAYSTACKS_TIMEOUT = 1000;
	const LS_KEY = 'yak_mania_instructions_breeder';

	let collected = $state(0);
	let show_instructions = $state(false);
	let floaters = $state([]);

	let hay_top = $state(0);
	let hay_left = $state(0);

	let game_area = $state(null);

	let hay_visible = $state(false);
	let completed = $state(false);
	let yak_invisible = $state(false);
	let yak_size_initial = $state(true);

	let timeout_id = null;

	let yak_sprite = $derived.by(() => {
		if (collected >= HAYSTACKS_TO_COLLECT) return hairyYak;
		if (collected >= HAYSTACKS_TO_COLLECT / 2) return youngYak;
		return babyYak;
	});

	const YAK_SIZE_CLASSES = [
		'h-[20vmin] w-[20vmin] max-h-24 max-w-24',
		'h-[23vmin] w-[23vmin] max-h-28 max-w-28',
		'h-[26vmin] w-[26vmin] max-h-32 max-w-32',
		'h-[29vmin] w-[29vmin] max-h-36 max-w-36',
		'h-[36vmin] w-[36vmin] max-h-40 max-w-40',
		'h-[40vmin] w-[40vmin] max-h-44 max-w-44',
		'h-[44vmin] w-[44vmin] max-h-48 max-w-48',
		'h-[48vmin] w-[48vmin] max-h-52 max-w-52'
	];

	let yak_size_classes = $derived.by(() => {
		if (collected >= YAK_SIZE_CLASSES.length || yak_size_initial) {
			return YAK_SIZE_CLASSES[YAK_SIZE_CLASSES.length - 1];
		}
		return YAK_SIZE_CLASSES[collected];
	});

	let game_area_w = $derived(game_area?.clientWidth);
	let game_area_h = $derived(game_area?.clientHeight);
	let ref_vmin = $derived(
		Math.min(document.documentElement.clientWidth, document.documentElement.clientHeight)
	);
	let hay_elem_size = $derived(Math.min(ref_vmin * 0.2, 112));
	let yak_elem_size = $derived(Math.min(ref_vmin * 0.48, 208));

	// Pre-define 4 zones around the center where haystacks can spawn.
	// Each zone is { top: [min, max], left: [min, max] }.
	let zones = $derived([
		// top
		{
			top: [hay_elem_size / 2, (game_area_h - yak_elem_size - hay_elem_size) / 2],
			left: [hay_elem_size / 2, game_area_w - hay_elem_size / 2]
		},
		// left
		{
			top: [
				(game_area_h - yak_elem_size - hay_elem_size) / 2,
				(game_area_h + yak_elem_size + hay_elem_size) / 2
			],
			left: [hay_elem_size / 2, (game_area_w - yak_elem_size - hay_elem_size) / 2]
		},
		// right
		{
			top: [
				(game_area_h - yak_elem_size - hay_elem_size) / 2,
				(game_area_h + yak_elem_size + hay_elem_size) / 2
			],
			left: [(game_area_w + yak_elem_size + hay_elem_size) / 2, game_area_w - hay_elem_size / 2]
		},
		// bottom
		{
			top: [(game_area_h + yak_elem_size + hay_elem_size) / 2, game_area_h - hay_elem_size / 2],
			left: [hay_elem_size / 2, game_area_w - hay_elem_size / 2]
		}
	]);

	let zone_order = [];
	let zone_index = 0;

	function shuffle_zones() {
		zone_order = [...Array(zones.length).keys()];
		for (let i = zone_order.length - 1; i > 0; i--) {
			const j = Math.floor(Math.random() * (i + 1));
			[zone_order[i], zone_order[j]] = [zone_order[j], zone_order[i]];
		}
		zone_index = 0;
	}

	function random_position() {
		if (zone_index >= zone_order.length) shuffle_zones();
		const zone = zones[zone_order[zone_index++]];
		const top = zone.top[0] + Math.random() * (zone.top[1] - zone.top[0]);
		const left = zone.left[0] + Math.random() * (zone.left[1] - zone.left[0]);
		return { top, left };
	}

	function spawn_haystack() {
		if (completed) return;
		const pos = random_position();
		hay_top = pos.top;
		hay_left = pos.left;
		hay_visible = true;

		clearTimeout(timeout_id);
		timeout_id = setTimeout(() => {
			hay_visible = false;
			spawn_haystack();
		}, HAYSTACKS_TIMEOUT);
	}

	function collect_haystack() {
		if (!hay_visible || completed) return;
		clearTimeout(timeout_id);
		floaters.push({ id: Date.now(), top: hay_top, left: hay_left });
		hay_visible = false;
		collected++;

		if (collected >= HAYSTACKS_TO_COLLECT) {
			completed = true;
			yak_size_initial = true;
			setTimeout(() => {
				yak_invisible = true;
				oncomplete?.();
			}, 600);
			return;
		}

		spawn_haystack();
	}

	onMount(() => {
		shuffle_zones();
		if (browser && !localStorage.getItem(LS_KEY)) {
			show_instructions = true;
		} else {
			spawn_haystack();
		}
		setTimeout(() => {
			yak_size_initial = false;
		}, 50);
	});

	function dismiss_instructions() {
		show_instructions = false;
		if (browser) localStorage.setItem(LS_KEY, '1');
		spawn_haystack();
	}

	onDestroy(() => {
		clearTimeout(timeout_id);
	});
</script>

<div class="relative flex flex-1 flex-col overflow-hidden">
	<div class="flex items-center gap-4 px-2 pt-2">
		<div class="flex-1 text-center">
			<p class="text-sm font-medium">Collect the hay bales to feed you yak!</p>
		</div>
		<div class="flex shrink flex-col items-center gap-1">
			<span
				class="flex items-center gap-2 rounded-full bg-base-200 px-4 py-1 text-xl font-semibold"
			>
				<img src={haySprite} alt="Hay" class="h-6 w-6 object-contain" />
				{collected} / {HAYSTACKS_TO_COLLECT}
			</span>
			<p class="text-sm opacity-50">Yak {yak.id.slice(0, 8)}</p>
		</div>
	</div>

	<div bind:this={game_area} class="game-area relative flex flex-1 items-center justify-center">
		<img
			src={yak_sprite}
			alt="Yak"
			class="{yak_size_classes} object-contain transition-[width,height,max-width,max-height] duration-500"
			class:invisible={yak_invisible}
			draggable="false"
		/>

		{#if hay_visible}
			<button
				type="button"
				class="absolute h-[20vmin] max-h-24 w-[20vmin] max-w-24 -translate-1/2 cursor-pointer"
				style="top: {hay_top}px; left: {hay_left}px"
				onclick={collect_haystack}
			>
				<img
					src={haySprite}
					alt="Haystack"
					class="h-full w-full object-contain"
					draggable="false"
				/>
			</button>
		{/if}

		{#each floaters as f (f.id)}
			<span
				class="pointer-events-none absolute text-4xl font-extrabold"
				style="top: {f.top}px; left: {f.left}px; animation: float-up-fade 700ms ease-out forwards"
				onanimationend={() => {
					floaters = floaters.filter((x) => x.id !== f.id);
				}}
			>
				+1
			</span>
		{/each}
	</div>
</div>

<CModal bind:open={show_instructions} id="breeder_instructions" onclose={dismiss_instructions}>
	<h3 class="text-lg font-bold">Breeder</h3>
	<p class="py-4">
		Your Yak is hungry! Tap the hay bales as they appear to feed it. They disappear quickly, so be
		fast! Collect {HAYSTACKS_TO_COLLECT} bales to complete the job.
	</p>
	<div class="modal-action">
		<button class="btn btn-primary" onclick={dismiss_instructions}>Understood</button>
	</div>
</CModal>
