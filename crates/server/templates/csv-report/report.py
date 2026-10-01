"""Summarize sales.csv: totals per region, written to summary.csv.

Croncave runs this on your computer. Anything it writes appears in Files with a tag
saying which run made it. Writing to $CRONCAVE_SUMMARY sets the headline you see on Home.
"""
import csv
import json
import os
from collections import defaultdict

totals = defaultdict(float)
rows = 0
with open("sales.csv", newline="") as f:
    for row in csv.DictReader(f):
        totals[row["region"]] += float(row["amount"])
        rows += 1

with open("summary.csv", "w", newline="") as f:
    w = csv.writer(f)
    w.writerow(["region", "total"])
    for region, total in sorted(totals.items()):
        w.writerow([region, f"{total:.2f}"])
        print(f"{region}: ${total:,.2f}")

best = max(totals, key=totals.get)
summary = {
    "headline": f"{rows} sales summarized; {best} sold the most",
    "values": {"Rows": rows, "Regions": len(totals), "Top region": best, "Total": f"${sum(totals.values()):,.2f}"},
}
path = os.environ.get("CRONCAVE_SUMMARY")
if path:
    with open(path, "w") as f:
        json.dump(summary, f)
print("Wrote summary.csv")
