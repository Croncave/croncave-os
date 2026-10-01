# Technical Architecture

2026-09-30 · @Treasure

Every computer is a small virtual machine that dials out to the platform over one connection, and every app, job and file travels over it. This doc covers how R1 is built and in what order. It follows the Product Definition; where they disagree, the product doc wins and this one is updated. Numbers marked [target] are to be confirmed by measurement.

**What R1 must do**

1. **Computers that sleep and wake.** A computer keeps its disk while asleep and wakes fast enough for its job [target: under 5 seconds].
2. **Outgoing only.** Every interaction with a computer travels over a connection it opens itself.
3. **Long unattended work.** Runs and agent tasks keep going for hours after the browser closes.
4. **Scheduled and triggered jobs** that wake the computer, run, record the result and let it sleep.
5. **Apps built on the platform.** Each app is written by hand for its purpose, on a shared platform that gives every app the same jobs, files, events, secrets and AI.
6. **Files people own.** An empty root they organise themselves, a Trash that catches every delete, and a record of which run made each file.
7. **Private previews** of anything a computer serves on localhost.
8. **AI that is optional.** Everything works with AI off. The assistant and agents use Claude and OpenAI when turned on.
9. **Isolation** strong enough for untrusted and AI-written code.
10. **US only** for sign-up, compute and data.

**Not in R1:** public hosting or incoming requests, GPUs, other regions, sharing (only the data model is ready), a mobile app, and running checks without waking the computer.

## System overview

*[Diagram: system overview · browser to computer through the edge and relay. See the doc on claude.ai for the drawing.]*

The browser only ever talks to the edge. The control plane decides what should happen; the relay carries it to each computer over the connection that computer's agent opened. Computers reach the internet through a filter, and built-in AI goes through one gateway that meters every call.

## Core objects

Everything belongs to an **account**, which is a single user in R1 and can become a group later without a migration. Every change records its **actor**: a user, the assistant, an agent, a schedule or the system.

| Object | Holds | Notes |
| --- | --- | --- |
| User | Sign-in identity, profile, preferences | Belongs to one or more accounts |
| Account | Computers, billing, spending cap, connections | Every user gets a personal account at sign-up |
| Computer | Account, name, size, disk size, state, sleep delay, compute reference | State follows the lifecycle below |
| App | Manifest: name, version, permissions, job kinds, events, worker | Built by us in R1; each app keeps its own data and records |
| Install | Computer, app, version, settings | One per app per computer |
| Type | App, version, config the app's own engine reads | Only for apps that offer types, Watcher first. Built-in, community or made by the user |
| Job | Install, kind, the setup (when, what, where results go, limits), status | Any app can offer job kinds; every run gets the same record |
| Run | Job, started by, trigger, status, start and end, exit code, awake seconds, AI cost, result summary | One per execution, including agent tasks |
| Event | Computer, actor, type, data, time | Feeds Activity, Home, notifications and the live UI |
| File record | Computer, path, created or changed by (run, agent, user), time | Where source tags come from; the bytes stay on the computer |
| Connection | Account, kind (GitHub, cloud drive, phone, AI sign-in), encrypted credential | Shared by every app that asks for it |
| Secret | Job or computer, name, encrypted value | Given to runs as environment variables, never logged |
| Usage | Computer, day, awake seconds by size, disk GB-hours, AI tokens, cost | Drives caps, the Usage page and billing |

## Computers

Each computer is a **microVM** with its own kernel, memory and a disk that is kept while it sleeps. That isolation is strong enough to run code the user didn't write, including code an agent wrote overnight.

The control plane manages computers through a **compute driver** with five operations: create, start, stop, status and destroy. Snapshot and resize come later. Everything else in the system talks to the driver, so the provider behind it can change.

| Size | Suits |
| --- | --- |
| Small | Monitoring, small scripts, file work |
| Medium | Most apps and one code agent at a time |
| Large | Heavy builds and several agents in parallel |

Small, Medium and Large are starting presets. Users can adjust CPU, memory and disk within their plan's limits, and a computer can change size while asleep.

*[Diagram: computer lifecycle · five states. See the doc on claude.ai for the drawing.]*

- **Wake triggers:** a scheduled run, an event trigger, the user opening an app or the Files app, or a visit to one of its previews. The target is under 5 seconds from trigger to ready.
- **Sleep:** about 30 seconds after nothing is active: no run, agent session, open app, Files view or preview. Before it sleeps, the orchestrator checks when the computer's next job is due. If that's within 5 minutes, the computer stays awake instead, because waking again would cost more [target: 5 minutes, tuned from measured wake costs]. Users can set a longer sleep delay per computer. Nothing is lost: the disk stays and work resumes on the next wake.
- **Keep awake:** for work that must stay up, such as a bot, the user can turn sleep off for a computer.
- **Billing follows state:** an awake computer is billed for compute by size plus its disk; an asleep one for its disk only.

## The outgoing connection

Each computer runs a small **agent**, a static binary started at boot. It opens one long-lived, encrypted connection to the relay on port 443 and carries everything over it as separate streams. Nothing on the computer listens for connections from outside.

**Transport.** A WebSocket on 443 with a stream multiplexer inside it. It passes through proxies that only expect web traffic, ends beside the app's own routes, and lets a large file transfer run without blocking a control message.

**Identity.** Each time a computer starts, the orchestrator gives it a one-time bootstrap token. The agent trades it for a short-lived credential that works for that one computer only, renewed while the connection stays up. Authorization happens before any stream opens.

| Stream | Direction | Used for |
| --- | --- | --- |
| Control | Relay to agent | Start or stop a run, apply settings, health checks |
| Events and logs | Agent to relay | Run output, progress, exit codes, file changes, result summaries |
| Files | Both | Browsing, uploads, downloads, previews of files |
| Web previews | Both | Browser traffic to a localhost port |
| Live view | Both | An agent's live session; a terminal later |
| Heartbeat | Both | Spotting a dropped connection quickly |

**Staying reliable**

- **Work never depends on the connection.** Runs are processes the agent supervises. If the connection drops, they keep going and the agent reconnects with backoff.
- **Logs and events are buffered on disk** until the relay acknowledges them, so a reconnect loses nothing.
- **Relays hold no state.** A routing table maps each computer to the relay node holding its connection, so any request can reach any computer.
- **Every stream is checked** against the account's permissions before it opens. The agent takes commands only from the relay.
- **Agent updates** are versioned and applied when a computer wakes.

## Apps on the platform

The platform is a small operating system, and apps are programs built on it. Every app does a different job in its own way (a watcher, a script runner, a coding workspace and a file browser have little in common), so each one is designed and written by hand for its purpose. What they share is the platform underneath: one way to run work, store files, report events, keep secrets and use AI.

**What an app is made of**

- **Screens** in the web app: the app's own code, built from the shared design system components (setup sections, summaries, run history, file lists, the chat panel and so on) so apps look and behave consistently. Screens reach the computer only through the platform API.
- **A worker** on the computer, when the app needs one: a program the agent starts, supervises and restarts. The Files app has none of its own; it is built into the agent.
- **A manifest** that registers the app with the platform: name, version, the permissions it needs, the job kinds it offers, the events it reports and its worker. The manifest describes the app to the platform; the app itself is code.

**The platform SDK.** What every app can use:

| Service | What it gives an app |
| --- | --- |
| Jobs and runs | Register job kinds, then schedule, trigger and run them with the same run record, limits and results |
| Files | Read and write in folders the person picked, a folder picker, and file records for everything it makes |
| App data | Private storage on the computer outside the person's files, plus its own records in the control plane |
| Events and notifications | Report what happened; the platform shows it in Activity and delivers notifications |
| Secrets and connections | Use a stored secret or a connection (GitHub, a cloud drive) the person approved |
| AI | Call the assistant through the AI gateway, only when the person has AI on |
| Previews | Show something served on localhost, privately |
| Settings | Per-install settings on the platform's standard setup screens |

**Apps in R1.** Files, Scripts, Watcher and Code, all built by us, each with its own worker:

| App | Worker on the computer |
| --- | --- |
| Files | Built into the agent: browsing, previews, Trash, zips |
| Scripts | Runs Python, Node.js and shell scripts, with packages installed per script |
| Watcher | The watcher engine, which runs checks for every watcher type |
| Code | Hosts the providers' own coding agents, unmodified |

**Isolation between apps.** Each app's worker runs as its own user on the computer. It can read its own app data and only the folders, connections and secrets the person granted it. Apps reach each other only through the platform.

**Types inside an app.** Some apps let people add variety without building a new app, mostly by describing it as data the app's own engine understands. Watcher is the first: each watcher type (stocks, flights, a web page) is a versioned config of inputs, source, fields, conditions and views, which the watcher engine runs as fetch, extract, compare and report. The assistant can draft a type; people see it as a normal setup screen and approve it before it runs. The platform provides the shared parts (schema validation, versions, test runs and approval) so another app can offer types too. Types are one app's choice, never how apps are built.

## Themes and color schemes

Two separate settings, both per user and synced across devices: the **mode** (light, dark or match the system) and the **color scheme** (the palette). R1 ships one scheme, the Croncave palette in light and dark, with dark as the default. The scheme picker comes later, so R1 builds everything to take any scheme without code changes.

- **Semantic tokens only.** Every color in the platform and in every app comes from a named token by role: surfaces, text, borders, accent, focus, the status colors (live, working, needs you, failed, asleep) and a set of chart colors. No hex values in product code; CI fails a change that adds one.
- **A scheme is data.** Each scheme is a versioned set of token values for both light and dark, stored like the plan catalog and added without a release.
- **Checked before it ships.** A scheme is validated automatically: text at least 4.5:1 against its surface, controls and focus rings at least 3:1, and status colors distinguishable from each other and from the accent, in both modes.
- **Status keeps its meaning.** A scheme can restyle the status colors but never swap their roles, and a status always carries its word as well as its color.
- **Applied before first paint.** The page loads with the user's scheme and mode already set on the root, so it never flashes the wrong theme.
- **Apps inherit it.** App screens use the shared components and tokens, so a new scheme reaches every app at once. An app can't define its own palette.
- **Outside the theme:** a user's own previews are never restyled, and emails use the default scheme.

## Jobs, schedules and runs

Every piece of work any app does becomes a run with the same record, whether it's a script, a check or an agent task. So Activity, logs, notifications, results and billing work the same way for every app.

**How a run happens**

1. A **trigger** fires: a schedule comes due, a user presses Run now, or the agent reports a file change in a watched folder. External triggers (a webhook or an email from another service, received by the platform, never by the computer) come later.
2. The control plane queues a run, keyed by job and trigger time.
3. The orchestrator makes sure the computer is awake, waking it if needed.
4. The control plane sends "start run" over the connection, with the job's setup, secrets and limits.
5. The agent hands the run to the app's worker, streams output and progress, and reports the result and exit code.
6. The run is closed, notification rules are checked, and, if nothing else is active, the computer sleeps about 30 seconds later unless its next job is due soon.

**Guarantees**

- **Each scheduled time runs once** from the person's point of view. A retry or duplicate can't create a second run for the same slot.
- **Missed runs run once on recovery,** not once per missed slot.
- **Overlap** follows the job's setting: skip the new run, or queue it.
- **Limits** end a run cleanly and record why: the agent enforces the longest run time, the control plane enforces spending.
- **Frequent jobs don't keep a computer awake all day.** A computer stays awake between runs only when the next one is due within 5 minutes; otherwise it sleeps and wakes again for the next run.

**The scheduler** is our own, on Postgres: a queue claimed with row locks and cron timing from a job library. No extra service to run.

**Results.** When a run ends, the agent reports three things:

- **The files it created or changed,** found by comparing the disk before and after the run (see Files).
- **The last lines of output** and the exit code.
- **An optional summary** the script writes to a file path the platform gives it: a few labelled values and a one-line headline. The headline is what Home and notifications show.

**Runs are linked.** Each run records what started it: a schedule, a person, an agent or another run. That costs nothing now and makes "run B with A's output" mostly a UI feature later.

## Files

Files live on the computer's own disk, and the Files app shows that disk as it is. There is no separate storage layer to keep in step.

**Layout on disk**

- **The person's root** is one folder, empty at first. Everything in it was put there by the person, or by a job saving where the person chose.
- Each app's own data (its history, logs, caches and agent transcripts) lives in a per-app system area outside the root and never shows in the tree.
- **The disk uses a copy-on-write filesystem,** so snapshots are instant and only store what changes.

**Trash that catches every delete**

- **Deletes from the Files app** move the item to a hidden trash area, with who deleted it and when.
- **Deletes by scripts and agents** are caught by a snapshot the agent takes at the start of every run. When the run ends, anything it removed is listed in Trash from that snapshot, attributed to the run.
- **Trash keeps items for 30 days** by default, then the snapshot is released. Trash space counts toward the disk, and the storage bar says how much it uses.

**Where files came from.** The same before-and-after snapshot tells the agent what each run created or changed. It sends those paths as events, and the control plane stores them as file records. Uploads, edits in the Files app and agent sessions are recorded the same way. That is where every source tag and "what this run made" list comes from.

**Moving files in and out.** Everything travels over the computer's connection, never a direct one.

- **Uploads** are chunked and resumable, so a large folder survives a dropped connection.
- **Downloads** stream from the computer; folders are zipped on the computer first.
- **Previews** of tables, images, PDFs and text are made on the computer and sent as small renders, so a 2 GB CSV never crosses the wire to show 20 rows.
- **Copying between computers** streams through the relay while both are awake (R2).
- **Cloud drives** are copied in on request or on a schedule by a runtime job, never mounted live (R2).

## Private previews

A preview lets the user use whatever their computer serves on localhost, as if it ran on their own laptop, over the same outgoing connection.

1. The agent notices a new port listening on localhost (say 5173) and reports it. The Code app shows "Running · open preview".
2. The person clicks it. The app mints a short-lived signed token for that computer and port and opens the preview's own address with it.
3. The preview address trades the token for a cookie scoped to that one preview.
4. Each request goes from the browser to the edge, then the relay, then down a preview stream to the agent, which forwards it to localhost.

**Rules**

- **A separate domain for previews,** one random subdomain each. Code people write runs there, so it must never share cookies or storage with the app's own domain.
- **It behaves like localhost.** The agent sends requests with a localhost host header, so dev servers that check it accept them. WebSockets and live reload pass straight through.
- **Only the user can open it in R1.** Sharing with signed-in people comes with sharing.
- **An open preview keeps the computer awake;** closing it lets the computer sleep.
- **Shown inside the app, with a new tab as the fallback.** Browsers restrict cookies in embedded frames, so this is tested early.

## AI

AI comes in two kinds, paid for in two ways. Both are off until the person turns them on, and turning them off stops every call to a provider.

|  | The assistant (in-app AI) | Agents (Code) |
| --- | --- | --- |
| What it does | Answers questions, proposes changes, helps set up apps | Works on code for hours: edits files, runs commands |
| Where it runs | Our control plane, calling the provider | On the computer, as the provider's own agent tool |
| Providers at launch | Claude and OpenAI | Claude and OpenAI agent tools |
| Who pays the provider | We do, and bill the person at exactly our cost | The person, through their own sign-in or API key |
| How it's capped | Tokens metered per call, stopped at the spending cap | Awake time within the cap; the provider's own limits for tokens |

**The assistant acts through the same API people use.** It never gets a back door. Every change it wants to make comes back as a proposal (the diff list in the chat panel), and applying it is a normal API call made by the person. Everything it does is recorded with the assistant as the actor.

**The AI gateway.** Every assistant call goes through one gateway in the control plane that:

- **Routes** each feature to a model, with a kill switch per model and provider.
- **Meters** tokens per call and prices them at the provider's rate.
- **Enforces** the account's spending cap before each call.
- **Keeps an evaluation set** per feature, so a model is only swapped after it passes.

**Agents on the computer**

- The providers' agent tools run **unmodified**, installed from their official sources.
- **With an API key:** the key stays in our control plane. Where the tool supports a custom endpoint, it points at our gateway, which adds the key and meters usage, so the key never sits on the computer. Otherwise the key is given to the run as an environment variable.
- **With a subscription sign-in:** the person signs in through the provider's own page, inside the computer. We never read, store or relay those credentials.
- **GitHub** access uses short-lived tokens for only the repositories the person picked, fetched fresh whenever git needs one.

Claude's terms allow its agent tool to run on a hosted computer with the user's own sign-in, so Claude is built first. OpenAI's terms need confirming before its agent is added. Open-source models come later, through the same gateway.

## Activity, notifications, usage and caps

Everything that happens on a computer becomes an **event**: a run started or finished, an app reported something, something needs the user, the computer woke or slept, a cap was reached. The computer's agent and the control plane write events to one append-only table in Postgres. Every surface the user sees reads from it.

- **Activity and Home.** The notifications panel (All and Unread) and Home's summary are queries over the event stream, filtered to what the user hasn't seen. The web app keeps a live subscription over its session connection, so new events appear without a refresh.
- **Delivery.** A notifier service reads new events and sends the ones the user asked for: email, web push in the browser, and text messages. Each app decides which of its events are worth a notification, and the user can change that per app and per channel. Repeats are grouped ("3 new price drops") so one busy app never floods a phone.
- **Quiet hours.** Non-urgent notifications wait until quiet hours end; events marked urgent go out straight away.

**Metering.** Usage is recorded as it happens, per computer and per app:

| Metered | Measured by | Shown as |
| --- | --- | --- |
| Awake time | Seconds in the awake state, by size, from the orchestrator's state changes | Hours awake |
| Storage | Disk GB per computer, sampled hourly, plus snapshot storage | GB stored |
| Built-in AI | Tokens in and out, per model, counted by the AI gateway | AI usage, in dollars |
| Outbound data | Bytes through the egress gateway | GB sent |

Meter readings roll up into hourly totals that feed the Usage page and the monthly invoice.

**Spending cap.** Every account has a monthly cap, set before anything runs, and each computer can have its own. The control plane checks spend against the cap as usage arrives:

1. At 50% and 80% the user gets an alert on every channel they've turned on.
2. At 100%, new runs are held, awake computers finish their current step and go to sleep, and the AI gateway stops accepting calls for that account.
3. Nothing is deleted. Raising the cap resumes held runs straight away.

## Plans and billing

Every commercial term lives in a **plan catalog**, stored as data in the control plane and changed without a deploy: plan prices, limits (computers, awake at once, largest size, fastest schedule, keep-awake, history), sign-up awards, monthly allowances, trial lengths and eligibility, promo codes, provider rates and markups. Nothing in code hard-codes a price or a limit.

- **Versioned.** Each change creates a new catalog version with a start date. An account stays on the version it signed up on until we move it, so existing users can be kept on old terms.
- **Overrides.** An account can carry its own overrides (a promo, a support credit, a higher limit), each with a reason, an actor and an optional end date.
- **One balance ledger.** Awards, monthly allowances, trial allowances, credits, usage and overage are entries in one append-only ledger per account. Usage draws from the award first, then the monthly allowance, then overage if the user opted in; the spending cap reads the same ledger.
- **Award rules.** One award per account, only for a direct sign-up, never for a trial or for keeping a plan after one, tied to the verified phone number and card, not the email.
- **Trials** switch the account to the trial plan's catalog entry with a pro-rated allowance, then switch back when they end unless the user chooses to stay. No charge happens automatically at the end.
- **Payments.** Stripe holds subscriptions and cards; our ledger works out usage and sends Stripe one invoice line per month, so each customer is charged once.

## Observability and measurement

Two jobs: keep the service healthy, and measure the things the product and pricing currently estimate, so the alpha replaces guesses with data.

**Keeping it healthy**

- Every service (control plane, relay, AI gateway, agent) emits structured logs, metrics and traces with a shared run and request id, so one run can be followed from the schedule to the computer and back. OpenTelemetry for collection; errors go to an error tracker.
- Dashboards and alerts track the targets: wake time, run start delay, notification delay, uptime, relay connections and queue depth.
- The agent reports a heartbeat and basic health for each computer.
- Secrets, tokens and keys are never logged.

**Measuring the estimates**

| Measured | How | Replaces the estimate for |
| --- | --- | --- |
| Wake cost | Orchestrator timestamps for trigger, ready and sleep, priced per size | The 30-second sleep rule and the 5-minute stay-awake threshold |
| Why a computer was awake | The agent reports what kept it awake: a run, an open app, a preview, an agent session or the stay-awake rule | Sleep tuning and usage profiles |
| Usage per account | The ledger: hours by size, disk, AI and data, and whether it came from the award, the allowance or overage | The model's usage profiles and allowances |
| Our real cost | A monthly job reconciles provider bills against our metering | The markup and every margin |
| Plan funnel | Billing events: sign-up plan, trial started and kept, upgrades, downgrades, cancellations, cap reached, overage turned on | Trial conversion, overage opt-in and plan mix |
| Activation | Apps installed, jobs created, time to first result, runs per job | Where new users get stuck |
| Abuse signals | Egress spikes, sustained CPU patterns, repeat sign-ups by phone or card | The free plan guardrails |

- Measurements record counts, durations and costs, never file contents, script output or prompts. Product analytics use aggregates; per-account detail is used only for billing and support.
- An internal dashboard compares measured values with the Plan Economics model's assumptions, and the model is updated from it before prices are fixed.

## Security and abuse controls

**Isolation.** Each computer is its own microVM with its own kernel, disk and network namespace. Computers accept connections only through the agent's outgoing link, so the attack surface is the relay, which authorizes every stream against the computer's account.

**Egress filtering.** All outgoing traffic from a computer passes a filter on the host:

- Cloud metadata endpoints, internal and private address ranges and other customers' computers are blocked.
- Outbound email ports (25, 465, 587) are closed; apps send email through the platform's notifier.
- Known mining pools are blocked, and sustained CPU patterns typical of mining are flagged for review.
- Bandwidth is limited per size and per plan, with alerts on sudden spikes.

**Secrets.** API keys, passwords and tokens that apps use are encrypted at rest with a per-account key held in a key management service. They're decrypted only on the computer, only for the run that needs them, and are masked in output and logs.

**Audit log.** Every change to a computer, an app, a job, a secret, a cap or a sign-in method is recorded with who made it (the user, the assistant or a schedule) and when. Users see their own audit log in Settings.

**Accounts.** Sign-up is verified by email and phone, with a US location check, which limits account farming. Each account has limits on how many computers it can create, how many can be awake at once and how often jobs can run.

**Apps and types.** In R1 every app is built and reviewed by us, and each app's worker runs as its own user with only what the user granted. In apps that offer types, a type is usually data the app's engine reads. An app can also let a type include code (a custom extraction step, say); that code runs inside the app's worker, with the same limits and permissions. A type made by the assistant or shared by the community is validated, shown to the user as a normal setup screen and needs their approval before it runs. Built-in types are reviewed and signed by us.

**Previews** run on a separate domain, one subdomain each, visible only to the user, so a preview can never read the main app's cookies.

## Stack and environments

A proposed stack, to confirm before the first line of code:

| Part | Proposed |
| --- | --- |
| Web app, app screens and shared components | TypeScript and Svelte/Sveltekit |
| Control plane, relay, agent, AI gateway | Rust, so the agent ships as one small static binary |
| Records, job queue and events | Postgres |
| Files, logs, snapshots, previews' static assets | S3-compatible object storage |
| Compute | Behind a driver interface: create, start, stop, status, destroy (snapshot and resize later) |

Nothing outside the compute driver depends on a specific provider, and computer images are standard images, so the provider can change without touching the rest.

**Environments**

- **Local:** one command starts Postgres, object storage, the control plane, the relay, the web app and computers on a local driver.
- **CI:** unit tests, a shared test suite every compute driver must pass, end-to-end tests in a real browser, and security tests (a computer can't be reached from outside; egress rules hold).
- **Staging:** its own accounts, domains and preview domain, test-mode billing and low-limit AI keys, with nightly end-to-end runs on real computers.
- **Production:** US regions only, invite-only for the alpha, new features behind flags, and agent updates rolled out to a few computers first.

## Build order for R1

Each step ends with a check that proves it works, and each is delivered in slices small enough to review in one sitting. Every step also ships its own telemetry and measurements (see Observability and measurement), so the alpha collects real data from its first day.

1. **Accounts and computers.** Sign-up with email and phone verification, sign-in, create and name a computer, and the theme tokens every screen uses. *Check: a new user creates a computer and sees it on Home.*
2. **Compute driver.** Local driver first, then the production provider. *Check: the shared driver tests pass on both; a computer starts, stops and keeps its disk.*
3. **Connection and agent.** The agent dials out, runs a command and streams output back; computers sleep when idle and wake on demand. *Check: a command runs on a sleeping computer within the wake target.*
4. **Files.** The Files app, uploads, previews and Trash, with per-run snapshots. *Check: upload a large file on a flaky connection, preview it, delete it and restore it.*
5. **Platform SDK, scheduler and runs.** App registration, jobs, schedules, run records, results and app data. *Check: a test app registers a job that** runs exactly once per slot across a control-plane restart.*
6. **Scripts app.** Its screens, its worker and the result view. *Check: a scheduled script's changed** fi**le**s a**nd summary appear on its result**.*
7. **Watcher app.** Its screens, the watcher engine, every built-in type, and assistant-made types with approval. *Check: each type finds a planted change on a test page**.*
8. **Home, Activity and notifications.** Event stream, panel, email, push and text. *Check: a**n alert from any app** reaches every turned-on channel once.*
9. **Code app.** Claude's coding agent with the user's own sign-in or key, then OpenAI's once its terms are confirmed, and private previews. *Check: an overnight session produces a change to review and a working preview.*
10. **Plans, billing and caps.** The plan catalog, the balance ledger, sign-up awards, trials, overage, Stripe, metering, the plan and usage pages, alerts and pausing at the cap. *Check: **usage draws from the award, then the allowance, then overage; a trial ends with no charge; **a computer sleeps at the cap and resumes when it's raised.*
11. **Hardening.** Egress rules, abuse limits, backups, monitoring and a security review. *Check: the security test suite passes and an outside review signs off.*

## Open decisions

- [ ] **Compute provider.** Leaning towards Fly; still comparing pricing, wake times and terms (including reselling compute) before choosing one for R1.
- [x] **Code agent sign-in.** Decided: Claude allows users to sign in to their own subscription on a hosted computer, so Claude's agent is built first. OpenAI's terms are confirmed before its agent is added.
- [x] **Size presets.** Decided: sizes are configurable. Small, Medium and Large are starting presets; users adjust CPU, memory and disk within their plan's limits.
- [ ] **Notification providers.** Suggested: Postmark or Amazon SES for email (Postmark for delivery quality, SES for cost at volume); Twilio for text messages, with US 10DLC registration done early since it takes weeks; web push sent by us through the browsers' standard push service, so no provider is needed. The email provider also sends sign-in links, and Twilio Verify sends one-time codes by text.
- [x] **Preview domain.** Likely `croncave``-previews.com`. The founder registers all domains.
- [x] **Third-party apps.** Decided: not in R1; possibly later, on the same platform SDK our apps use.
- [x] **Stack.** Confirm the proposed languages and libraries.
- [x] **Targets.** Proposed for the alpha: wake under 5 seconds for 95% of wakes; scheduled runs start within 15 seconds of their time for 95% of runs; notifications sent within 15 seconds of the event (a soft target); web app and control plane up 99.5% of the time (99.9% at launch); the database restorable to any point with at most 5 minutes of data lost and back within 1 hour.
- [ ] **Observability backend.** Which service stores logs, metrics and traces, and which tracks errors.
