# Design

The designs live on claude.ai design canvases (private to the founder's account):

- Platform screens (Home, notifications, computer switcher, new computer, settings, Files):
  https://claude.ai/artifact/QZHhozWVvtXLPx8CPYLU9e
- Watcher (setting up a page watch, the watch page, the assistant on a watch, stock watch setup and page, the type
  gallery, a setup built from a type's config, a watch set up by the assistant): https://claude.ai/artifact/5TWMR7ksbdKWZydVu2qLHp
- Scripts (a scheduled script, a run in progress, a finished run and the files it made, adding a script, a failed run):
  https://claude.ai/artifact/Tb9kNmT6bEEoix7rwYp6XB
- Code (the editor with the agent working, reviewing the agent's changes, the live preview, agent tasks that run while
  you're away):
  https://claude.ai/artifact/N7kp6tVJQutLo5DD2PTFmC
- Sign in and account (sign in, create an account, check your email; the email link continues to Plans and Billing).
  The right-hand panel shows the Home "Since you left" card as the example of what Croncave does while you're away:
  https://claude.ai/artifact/1zngkuwQZW6NP5SRVVTWMw
- Plans and Billing (sign-up: your details, time zone picker listing all US time zones, and mobile number; confirm the
  phone code; plans; trial offer, plan and usage, spending and overage, usage ran out,
  trial ending): https://claude.ai/artifact/RubDfGiospYk7fYPEpgoAD
- Pricing model (interactive): https://claude.ai/artifact/QqDfEEf1QZ8cDMu4nkFiV8

## Look

Dark by default, with light and "match system". Croncave dark tokens:

| Token | Dark | Light | Role |
| --- | --- | --- | --- |
| bg | #0b0b0d | #f7f7f8 | Page background |
| surface | #131316 | #ffffff | Windows and cards |
| pane | #0e0e11 | #f1f1f3 | Sunken panes, side panels |
| line | #26262c | #e4e4e7 | Borders |
| ink | #ededf0 | #111113 | Text |
| mid | #a1a1aa | #5b5b66 | Secondary text |
| low | #85858f | #6b6b75 | Labels |
| accent | #99d52a | #18181b | One primary action per view, current marker |
| on-accent | #1a2e05 | #ffffff | Text on accent |
| accent-ink | #b5e35c | #3f6212 | Links, "live" status |
| accent-soft | #1c2708 | #ecfccb | Soft accent fill |
| working | #7aa7ff | #1d4ed8 | "Working" status |
| needs | #fb923c | #9a5b00 | "Needs you" status (light changed 2026-10-01, see decisions) |
| failed | #f87171 | #b91c1c | "Failed" status |
| asleep | #a1a1aa | #52525b | "Asleep" status |

Fonts: Inter for UI, JetBrains Mono for labels, times, costs and counts. Layout: a top bar (computer switcher with
its state, usage left, account) and a left dock of apps (Home, Files, Activity, then installed apps, All apps, Get
apps, Settings at the bottom). Statuses always pair a color with a word. Colors are semantic tokens only, so color
schemes can be added as data later.
