export type Status =
	| 'awake'
	| 'asleep'
	| 'waking'
	| 'sleeping'
	| 'working'
	| 'needs_you'
	| 'creating'
	| string;

export type Computer = {
	id: string;
	name: string;
	size: string;
	cpu: number;
	memory_gb: number;
	disk_gb: number;
	state: string;
	status: Status;
	status_word: string;
	running: number;
	connected: boolean;
	keep_awake: boolean;
	sleep_delay_secs: number;
	health: { cpu_percent?: number; memory_mb?: number; disk_bytes?: number; trash_bytes?: number };
	next_job: { at: string; name: string } | null;
	open_ports: number[];
	note: string | null;
	agent_version: string | null;
	created_at: string;
	state_changed_at: string;
	time_zone: string;
};

export type Me = {
	user: {
		id: string;
		email: string;
		name: string;
		phone: string | null;
		time_zone: string;
		is_admin: boolean;
		prefs: Record<string, any>; // eslint-disable-line @typescript-eslint/no-explicit-any
		previous_seen_at: string | null;
	};
	account: {
		id: string;
		plan: string | null;
		trial_plan: string | null;
		trial_ends_at: string | null;
		ai_enabled: boolean;
		paused_at: string | null;
		signup_completed_at: string | null;
	};
	signup_step: 'phone' | 'plan' | 'trial' | 'done';
	computers: Computer[];
	unread: number;
	usage: { left_micros: number; spent_this_month_micros: number; cap_micros: number; plan_name: string } | null;
	dev_tools: boolean;
	now: string;
};

export type EventRow = {
	id: number;
	computer_id: string | null;
	actor_kind: string;
	app: string;
	kind: string;
	level: string;
	title: string;
	body: string;
	data: Record<string, unknown>;
	run_id: string | null;
	job_id: string | null;
	read_at: string | null;
	created_at: string;
};

export type RunBrief = {
	id: string;
	status: string;
	status_note: string | null;
	trigger: string;
	headline: string | null;
	queued_at: string;
	started_at: string | null;
	ended_at: string | null;
	error_plain: string | null;
	summary: { headline: string; values: { label: string; value: string }[] } | null;
	attempt: number;
	started_by: string;
	data: Record<string, any> | null; // eslint-disable-line @typescript-eslint/no-explicit-any
};

export type Job = {
	id: string;
	computer_id: string;
	app: string;
	kind: string;
	name: string;
	setup: Record<string, any>; // eslint-disable-line @typescript-eslint/no-explicit-any
	trigger: string;
	schedule: string | null;
	schedule_words: string | null;
	watch_path: string | null;
	overlap: string;
	max_runtime_secs: number;
	retries: number;
	max_spend_micros: number | null;
	notify: Record<string, boolean>;
	status: string;
	next_due_at: string | null;
	last_run: RunBrief | null;
	secret_names: string[];
	runs?: RunBrief[];
	rule?: string;
};
