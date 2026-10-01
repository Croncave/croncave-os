# Product Definition

2026-09-30 · @Treasure

Croncave gives anyone a private computer in the cloud that keeps working while they're away, used through simple apps in any browser.

This doc defines the product in two parts. An Amazon-style **press release and FAQ** sets out the value. A **story map** lists every feature as a user story, with a **MoSCoW** priority and a release.

- **Croncave** is the product's name.
- **MoSCoW is judged against the public launch.** Must = launch can't happen without it. Should = launch if at all possible. Could = welcome after launch. Won't = deliberately not now, but the design must leave room for it.
- **Releases:** R1 is a private alpha, R2 is the public launch, R3 is the first expansion, Later is everything after.
- Anything not yet decided is listed under Open decisions, not guessed.

How it is built is covered in the Technical Architecture; pricing is in the Pricing section below.

## Press release

*Written as if the public launch (R2) has happened. Quotes are hypothetical, as the format intends.*

### Croncave launches: a computer in the cloud that keeps working after you close your laptop

**Watch prices and listings, run your scripts on a schedule, and hand coding work to an agent, all on a private computer that runs while you sleep. Ready in minutes, and simple enough for anyone.**

**[CITY], [LAUNCH DATE].** Today Croncave opens to everyone. It gives people their own computer in the cloud and a set of simple apps to use it. Set something up in a few minutes, close your laptop, and come back to results in plain language.

**The problem.** Plenty of useful work needs a computer that stays on. Checking an apartment site every 15 minutes, backing up files every night, running a long data job, or letting an AI agent keep coding. Today that means leaving a laptop open or renting a server, which means picking a provider, sizing a machine, installing tools and reading logs. Companies have engineers for this. Everyone else goes without.

**The solution.** Croncave gives you a private computer that sleeps when idle and wakes when there's work. You use it through simple apps:

- **Watcher** checks a page, a stock, a flight or a forecast on a schedule and tells you when something changes.
- **Scripts** runs your own Python, JavaScript or shell scripts by hand, on a schedule, or when files change.
- **Code** lets you edit a project and hand an AI agent a task that keeps going after you leave. You review every change before it lands.
- **Files, Home and Activity** keep your files, show what happened while you were away, and tell you when something needs you.

Every app works without AI. If you want help, turn on the assistant and ask it to set things up. It fills in the same forms you would, and nothing starts until you approve.

**Private by design.** Your computer only makes outgoing connections, so only you can reach it, through Croncave. You see what each job costs before it runs, and everything pauses at the monthly spending cap you set.

> "I used to leave my laptop open all night so my scraper would keep running. Now it just runs, and I check the results over breakfast." [HYPOTHETICAL CUSTOMER], small business owner

> "We wanted people who aren't engineers to have what engineers have: a computer that works for them around the clock." [FOUNDER NAME], founder of Croncave

**Getting started.** Sign up at [URL], pick a starter computer, and set up your first watch or script in under five minutes. Start free, or pick a plan from $10 a month. Every plan comes with free usage, and you choose a spending cap.

## Customer FAQ

**What exactly is my computer?** A private computer in the cloud with its own files, apps and storage. You open its apps in your browser, like apps on a phone, and they show you what the work produced.

**What can I do with it?** Anything that needs a computer to stay on: watch prices, listings, stocks or appointment slots; run scripts on a schedule; crunch a large spreadsheet; back up files; or let an AI agent work on your code overnight.

**Do I need to know how to code?** No. Watcher, Home, Files and the other built-in apps work with plain forms: pick what to watch, when, and how to tell you. If you can code, the Scripts and Code apps run your own code.

**Do I have to use AI?** No. Everything works with AI off, and AI is off until you turn it on. When it's on, the assistant fills in the same forms you would and shows you the change before anything runs.

**Which AI does it use, and who pays?** Claude and OpenAI at launch. The assistant and other in-app AI features are billed at exactly what the AI provider charges us, and stop at your spending cap. For agent work like coding, you sign in with your own Claude or OpenAI account or bring your own API key, and the provider bills you directly.

**What happens when I close my laptop?** Nothing changes. The work runs on your computer in the cloud, not your laptop. When something finishes or needs you, you get a notification. Open Croncave on any device to pick up where things stand.

**Who can reach my computer?** Only you. It makes outgoing connections only, so it has no public address. Apps you build on it, like a website you're coding, open privately for you through Croncave.

**Can I build a website on it?** Yes. You can run and preview it privately, just like on your own laptop. When it's ready, push the code to GitHub and publish it on the host of your choice.

**How do I know what it did?** Home shows what happened since you left, in plain words ("2 new apartments matched your watch"). Each app shows its own history, results and files. Raw logs are one level deeper for anyone who wants them.

**What does it cost?** There's a free plan, and paid plans at $10, $25 and $60 a month. Each plan comes with usage for the hours your computer is awake and the storage it keeps. Your computer sleeps when nothing is running, so a check that runs every hour costs pennies. You set a monthly spending cap and everything pauses there, so you never get a bill you didn't choose. New accounts can try the next plan up for free.

**Can I have more than one computer?** Yes. You might keep a small one for watches and a big one for heavy jobs. Each has its own size, files and apps.

**What is it best for?** Work that runs on its own: watching for changes, scheduled scripts, long data jobs and agent tasks. For linking hundreds of online services together, a dedicated automation tool is the better fit.

**Can I share it with someone?** Not at launch. Sharing a computer with a partner, co-founder or team is planned, and the product is designed for it from day one.

**Where is it available?** The United States at launch. Other regions follow demand.

## Internal FAQ

**What principles must every feature respect?**

1. **Apps, not machines.** People see their computer, its apps and their results. CPUs, memory and disks appear only as plain presets, with exact specs one level down.
2. **Outgoing only.** Computers only make outgoing connections. Previews, files, logs and live views all travel through the platform.
3. **Complete without AI.** Every job can be set up, run and fixed with AI off. The assistant is opt-in, fills in the same forms a person would, and never acts without approval.
4. **Apps built for their purpose.** Each app is written by hand for the job it does, on a shared platform and shared design components, so apps work differently but look and behave consistently. Some apps, like Watcher, let people and the assistant add types as config; that is an app's choice, not how apps are built.
5. **Plain words, detail on demand.** Every screen has a default layer, a details layer and an expert layer.
6. **Sleep by default, cost up front.** A computer is awake only while work runs or someone is looking. Anything that keeps it awake says what it will cost first.
7. **One place for what happened.** Every app reports through the same Activity, notifications and Home, so people never check app by app.
8. **Ready for sharing from day one.** Computers belong to an owner that can later be a group, and every action records who or what did it: a person, the assistant or a schedule.

**Who is the first customer?** People who already have work that needs a computer to stay on: busy technical people, founders and small business owners with a script, a monitor or an agent task they run by hand today. They feel the pain most and tolerate rough edges. Students and non-technical users are the goal, and arrive through Watcher types and templates in R2 and R3.

**Why apps?** Apps show each kind of work in the form that suits it: a price chart for a stock, match cards for listings, a diff for code. They're easy for anyone, work well on a phone, and are made for checking on work that happened while you were away. A terminal arrives later as one more app, for people who want it.

**Who do we compete with?** Three groups, none aimed at our customer:

- **Cloud dev environments and coding agents** serve engineers and teams, not people who just want work to keep running.
- **Automation and monitoring tools** handle narrow jobs well but can't run your own code or long tasks.
- **Cloud providers and hosting platforms** can do anything, but expect you to manage servers.

Our angle: one private computer for all of it, usable without technical knowledge, with apps built for each kind of work. [A competitive scan with sources is still to do.]

**What are the biggest risks?**

- **Abuse:** crypto mining, spam, scraping sites that forbid it, bots at scale. Mitigations: egress filtering, per-account limits, verified accounts, clear terms.
- **Always-awake cost:** frequent watches and long jobs keep a computer awake. It must be shown up front, and frequent jobs should let the computer sleep between runs.
- **Watchers breaking:** sites change and scraped values go missing. Every watch needs a visible "couldn't check" state and a repair path.
- **Agent quality:** a bad overnight agent session damages trust. Agents work on branches, changes are reviewed before they land, and nothing merges on its own by default.
- **Shared and agent-made watcher types:** config from strangers or from an agent could read the wrong things or leak data. Every type shows what it reads and from where, runs a test first, and waits for approval.
- **Money-moving automations** (trading, payments): out of scope until limits, confirmations and legal review exist.

**What do we focus on?** Private computers that do work for one person, through apps. We leave public hosting, app-integration hubs and team development environments to products built for them.

## Story map: backbone

Every person takes the same path, with or without AI. Every feature in the next section sits under one of these activities.

*[Diagram: story map backbone · 6 activities, 3 layers. See the doc on claude.ai for the drawing.]*

Start and Set up happen once per job. Run, Watch, Review and Adjust repeat for as long as it runs. The assistant, Manage and Share apply at every step.

## Story map: every feature

Each row is one thing a person can do, written from their side. Priority is judged against the public launch (R2). Both columns are dropdowns, so re-prioritise in place.

### Start

| Story: I can… | Priority | Release |
| --- | --- | --- |
| Sign up with my email and a sign-in link, no password | Must | R1 |
| Create my first computer from a starter: Small, Medium or Large | Must | R1 |
| Create a computer from scratch, choosing its size, storage and starting apps | Must | R1 |
| See what a computer costs awake and asleep before I create it | Must | R1 |
| Upload files or a folder, or pick them from a connected drive | Must | R1 |
| Bring in a project from GitHub | Must | R1 |
| Have more than one computer and switch between them from the top bar | Should | R1 |
| Add more apps from the app gallery | Must | R2 |

### Set up

| Story: I can… | Priority | Release |
| --- | --- | --- |
| Set up a watch by picking a type (web page, stocks, flights and more) and filling in a form | Must | R1 |
| Add my own script and have its packages found from requirements.txt or package.json | Must | R1 |
| See every setup in plain words before I save it | Must | R1 |
| Test any job and see real results before it goes live | Must | R1 |
| Choose when a job runs: by hand, on a schedule, or when files change | Must | R1 |
| Choose how I'm told: in the app and by email | Must | R1 |
| Also be told by push notification or text message | Should | R2 |
| Set quiet hours and a daily limit on alerts | Should | R2 |
| Set limits on a job: longest run, retries, what it may spend | Must | R1 |
| Add secrets that reach my scripts as environment variables | Must | R1 |
| Make my own watcher type from a config file | Could | R3 |
| Use watcher types made by others, after seeing what they read and a test run | Could | R3 |

### Run

| Story: I can… | Priority | Release |
| --- | --- | --- |
| Rely on my computer sleeping when idle (about 30 seconds after nothing is active, or later if I choose) and waking for work, with my files kept | Must | R1 |
| Have scheduled jobs wake the computer, run, and let it sleep again | Must | R1 |
| Press Run now or Check now on any job | Must | R1 |
| Keep a long job running after I close the tab, with live progress and resource use | Must | R1 |
| Stop a running job | Must | R1 |
| Choose whether a job may overlap with its last run | Must | R1 |
| Keep the computer awake for a job, with the monthly cost shown first | Should | R2 |
| Run light checks without waking the computer | Could | Later |
| Use computers with a GPU | Won't | Later |
| Let automations move money, such as trades or payments | Won't | Later |

### Watch

| Story: I can… | Priority | Release |
| --- | --- | --- |
| Open Home and see a greeting, a command bar and what happened since I left | Must | R1 |
| See my computer's state, CPU, memory, disk and this month's spend in the top bar | Must | R1 |
| Open Activity as a side panel, showing all or only unread | Must | R1 |
| Get notified when something finishes, finds something or fails | Must | R1 |
| Watch a running job's live output from any device | Must | R1 |
| Search files and apps | Must | R2 |
| Run a command, from the command bar | Should | R3 |
| Use the whole product comfortably in my phone's browser | Should | R3 |
| Get push notifications from a mobile app | Could | Later |

### Review and use

| Story: I can… | Priority | Release |
| --- | --- | --- |
| See each run's result: a summary, the files it made and its output | Must | R1 |
| Preview tables and images from a run, and open them in the right app | Should | R2 |
| Browse, upload, download and delete files | Must | R1 |
| See a watch's history at a glance: every check, match and failure | Must | R1 |
| Open an app running on my computer in a private preview, at laptop or phone size | Must | R1 |
| Review an agent's changes file by file, keep or undo each, then open a pull request | Must | R1 |
| Compare a run with earlier runs | Could | R3 |

### Adjust

| Story: I can… | Priority | Release |
| --- | --- | --- |
| Change a job in its form, or by clicking a word in its plain-words rule | Must | R1 |
| Pause, resume or delete any job | Must | R1 |
| See why a run failed in plain words, with the fix beside it | Must | R1 |
| Retry a failed run | Must | R1 |
| Resize a computer or add storage | Must | R1 |
| Restart, reset or delete a computer | Must | R1 |
| Edit code in the Code app, with a file tree and syntax highlighting | Must | R1 |
| Open a terminal on my computer | Could | R3 |

### Assistant (opt-in)

| Story: I can… | Priority | Release |
| --- | --- | --- |
| Turn the assistant on or off for my account or for one computer | Must | R2 |
| Ask the assistant from any app, and have it know what I'm looking at | Must | R2 |
| See the assistant's proposed changes as a list I can apply or reject | Must | R2 |
| Describe a watch in my own words and get a filled-in setup to review | Should | R2 |
| Hand the Code agent a task that keeps running while I'm away | Must | R1 |
| Approve or deny each command the agent wants to run | Must | R1 |
| See everything the assistant did in Activity | Must | R2 |

### Manage

| Story: I can… | Priority | Release |
| --- | --- | --- |
| See usage by day, by computer and by app | Must | R1 |
| Set a monthly spending cap, get alerts at 50% and 80%, and have work pause at the cap | Must | R1 |
| Keep connections in one place: GitHub, cloud drives, phone number | Must | R1 |
| Use dark mode by default, or switch to light or match my system | Must | R1 |
| Pick a color scheme, not just light or dark | Should | R2 |
| Pay for a plan and download invoices | Must | R2 |
| Manage my profile, signed-in devices and two-step sign-in | Must | R2 |
| Export or delete all my data | Must | R2 |
| Choose the region my computers run in | Could | Later |

### Share

| Story: I can… | Priority | Release |
| --- | --- | --- |
| Have my computers belong to an owner that can later be a group (data model only, no screens) | Must | R1 |
| See who or what made each change: me, the assistant or a schedule | Should | R2 |
| Share a computer with other people, with roles | Should | Later |
| Share a private preview with specific signed-in people | Could | Later |

## Release slices

R1 is the thinnest version that proves the core promise: set up a job or hand off a coding task, close the laptop, and come back to a result you can trust. Each later release starts only after its gate is passed.

*[Diagram: release slices · 4 phases, 3 gates. See the doc on claude.ai for the drawing.]*

Gate 1 is a proposal to confirm. Gates 2 and 3 are set once the release before them has taught us what to measure.

**Won't have for now.** Saved for later on purpose. The design leaves room for each:

- Public hosting of sites, and incoming requests
- An app-integration hub in the style of Zapier
- Computers with a GPU
- Automations that move money, such as trades or payments
- Apps that draw their own screens outside the building blocks

## Pricing

Croncave charges a monthly plan plus usage. The plan decides what you can do; usage is what your computers consume. Every figure here is a starting point to test in the alpha, and every one is configurable in the plan catalog (see the Technical Architecture), so changing a price, limit, award, trial or promo needs no release. The numbers behind this section are in the [Plan Economics model](https://claude.ai/artifact/QqDfEEf1QZ8cDMu4nkFiV8).

|  | Free | Plus | Pro | Max |
| --- | --- | --- | --- | --- |
| Price | $0, no card | $10 a month | $25 a month | $60 a month |
| Sign-up award, one time | $2 | $5 | $10 | $25 |
| Monthly allowance, once the award is spent | $1 | $3 | $5 | $15 |
| Computers | 1 | 2 | 5 | 10 |
| Awake at the same time | 1 | 1 | 3 | 8 |
| Largest size | Small | Medium | Large | Large or custom |
| Most frequent schedule | Hourly | Every 5 minutes | Every minute | Every minute |
| Keep-awake computers | None | None | 1 | 5 |
| Trial offered at sign-up | 14 days of Plus | 7 days of Pro | 7 days of Max | None |
| Past the allowance | Work pauses | Opt in, with your own cap | Opt in, with your own cap | Opt in, with your own cap |
| Code agents, with your own sign-in or key | Yes | Yes | Yes | Several in parallel |
| History kept | 7 days | 30 days | 30 days, adjustable | 90 days, adjustable |
| Support | Help docs | Email | Email | Priority email |

**How usage is billed**

- Shown in plain units: dollars, hours awake and GB stored.
- Usage draws from the sign-up award first, then the monthly allowance, then overage for users who opted in.
- Compute, disk and outbound data are billed at our provider's cost plus 25% [proposed]. Built-in AI is billed at exactly our cost. Code agents run on the user's own Claude sign-in or key, so we never bill their tokens.
- The spending cap starts at the allowance. Alerts go out at 50%, 80% and 100%; at the cap, computers sleep and nothing is deleted.
- Computers sleep about 30 seconds after nothing is active, and stay awake if their next job is due within 5 minutes.

**Awards and trials**

- The sign-up award goes only to direct sign-ups, once per account, for the plan chosen at sign-up. Trials never come with an award, and neither does keeping a plan after a trial. Awards are tied to the verified phone number and card, not the email.
- Free accounts get 14 days of Plus; paid accounts are offered 7 days of the next plan up. A trial has the trial plan's limits and a pro-rated monthly allowance, then goes back to the original plan unless the user chooses to stay. Nothing is charged automatically when a trial ends. One trial per verified phone number.

**Free plan guardrails**

- No card, and work pauses at the allowance, so a free account never becomes a bill.
- Small computers only, one awake at a time, schedules no more often than hourly, no keep-awake.
- Phone verification at sign-up; mining pools and outbound email ports blocked; lower bandwidth limits.
- Free computers unused for 60 days are deleted, after two email warnings.

**What the model shows** with its starting assumptions:

|  | Free | Plus | Pro | Max |
| --- | --- | --- | --- | --- |
| Contribution per user, per month | −$0.56 | $6.76 | $20.47 | $49.85 |
| Margin | — | 66% | 64% | 54% |

- Break-even at about 840 accounts, with $1,500 a month of fixed platform costs, 85% of accounts on Free, and paid accounts split 55/35/10 across Plus, Pro and Max. About $7,400 a month of profit at 5,000 accounts.
- Trials pay back in under a month; the Free to Plus trial is nearly all of it.
- Pro and Max margins depend on heavy users paying overage: a typical Max user's bill comes to about $92. Watch whether heavy users see that as fair.
- The usage profiles, overage opt-in and trial conversion rates are estimates. The alpha measures each of them (see Observability and measurement in the Technical Architecture) and the model is updated with real numbers before launch.

**Still to test:** the price points, Max's monthly allowance ($15 or $25), annual plans and a student discount.

## Open decisions

Ten of eleven questions are decided. Gate targets are still open.

- [x] **Name.** Decided: Croncave. The founder registers the domains.
- [x] **First customers.** Decided: the alpha users are already lined up.
- [x] **Launch regions.** Decided: US only at launch, for sign-up, computers and data. Other regions follow demand.
- [x] **AI providers and billing.** Decided: Claude and OpenAI at launch, with open-source models later. On whether people bring their own key or subscription, or we bill AI at cost: in-app AI such as the assistant is billed at exactly what the provider charges and stops at the spending cap. Agent work such as coding uses the person's own sign-in or API key, billed by the provider.
- [x] **Code in the alpha?** Decided: yes. Coding is one of the alpha users' requests, so the Code app and its agent are in R1.
- [x] **Watcher types at launch.** Decided: every type in the gallery ships at launch.
- [x] **Computer sizes.** Exact CPU, memory and disk behind Small, Medium and Large, to be set from real usage and cost.
- [x] **Sleep defaults.** Decided: a computer sleeps about 30 seconds after nothing is active, and stays awake if its next job is due within 5 minutes. People can set a longer delay per computer.
- [x] **Retention.** Decided: 30 days by default, and people can change it.
- [x] **Mobile.** Decided: web only. A mobile app waits until usage proves it's needed.
- [ ] **Gate targets.** The evidence needed before each release starts (below).
