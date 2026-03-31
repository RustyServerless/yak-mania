import { SvelteMap } from 'svelte/reactivity';
import { v4 as uuidv4 } from 'uuid';

export const alerts = new SvelteMap();

export function alert(title, alert_class, message, timeout) {
	const id = uuidv4();
	const timer_handler = setTimeout(() => {
		alerts.delete(id);
	}, timeout);
	const error_wrapper = {
		title,
		alert_class,
		message,
		timer_handler,
		close: () => {
			clearTimeout(timer_handler);
			alerts.delete(id);
		}
	};
	alerts.set(id, error_wrapper);
}

export function alert_error(message, timeout = 2500) {
	alert('Error', 'alert-error', message, timeout);
}
export function alert_success(message, timeout = 2500) {
	alert('Success', 'alert-success', message, timeout);
}
export function alert_warning(message, timeout = 2500) {
	alert('Warning', 'alert-warning', message, timeout);
}
export function alert_info(message, timeout = 2500) {
	alert('Info', 'alert-info', message, timeout);
}

export function alert_appsync_error(appsync_error_response, message, timeout = 5000) {
	console.error(appsync_error_response);
	for (const error of appsync_error_response.errors) {
		let error_type;
		if (error.errorType) {
			if (error.errorType.startsWith('Lambda:')) {
				error_type = `[${error.errorType.substring(7)}] `;
			} else {
				error_type = `[${error.errorType}] `;
			}
		} else {
			error_type = '';
		}
		const error_message = error.message;
		alert_error(`${message} (${error_type}${error_message})`, timeout);
	}
}
