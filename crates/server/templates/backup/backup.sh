#!/usr/bin/env bash
# Back up a folder into Backups/, one archive per run, keeping the newest 5.
# Change SOURCE to the folder you want to keep safe.
set -euo pipefail
SOURCE="${SOURCE:-Scripts}"
cd "$CRONCAVE_FILES"
mkdir -p Backups
name="Backups/$(basename "$SOURCE")-$(date -u +%Y%m%d-%H%M%S).tar.gz"
tar -czf "$name" "$SOURCE"
echo "Saved $name ($(du -h "$name" | cut -f1))"
ls -1t Backups/*.tar.gz | tail -n +6 | xargs -r rm --
count=$(ls -1 Backups/*.tar.gz | wc -l | tr -d ' ')
printf '{"headline":"Backed up %s","values":{"Archive":"%s","Backups kept":"%s"}}' "$SOURCE" "$name" "$count" > "$CRONCAVE_SUMMARY"
