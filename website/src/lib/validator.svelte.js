export class ValidatedValue {
	value = $state(null);
	error = $derived.by(() => {
		for (const rule of this.rules) {
			const rule_res = rule(this.value);
			if (rule_res != true) {
				return rule_res;
			}
		}
		return null;
	});
	in_error = $derived(this.error != null);
	is_empty = $derived(this.value == null || this.value == '');
	would_display_error = $state(false);
	display_error = $derived(this.in_error && this.would_display_error);
	timeout = null;

	constructor(rules) {
		this.rules = rules;
		$effect(this.delay_effect);
	}

	delay_effect = () => {
		if (this.value != null) {
			this.would_display_error = false;
			this.timeout = setTimeout(() => {
				this.would_display_error = true;
			}, 1000);
			return () => {
				clearTimeout(this.timeout);
			};
		}
	};

	display_error_now = () => {
		this.would_display_error = true;
	};

	reset = () => {
		this.would_display_error = false;
		this.value = null;
		clearTimeout(this.timeout);
	};
}
