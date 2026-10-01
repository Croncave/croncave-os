# Design

The designs live on claude.ai design canvases (private to the founder's account):

- Platform screens (Home, notifications, computer switcher, new computer, settings, Files):
  https://claude.ai/artifact/bec448a6-b6e1-477b-9aea-966e600793a9
- Plans and Billing (sign-up with phone, plans, trial offer, plan and usage, spending and overage, usage ran out,
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
