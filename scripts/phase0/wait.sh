#!/usr/bin/env bash
# Wait up to about nine minutes for a desktop-session job, then print its log.
# Exit code 3 means it is still running: run this again.
set -euo pipefail
HOST="${LAZYMKV_BOX:-USER@makemkv-box}"
LOG="C:\\lazymkv\\phase0\\logs\\$1.txt"
for _ in $(seq 1 52); do
  if ssh -o BatchMode=yes "$HOST" "findstr /b PHASE0-DONE $LOG" >/dev/null 2>&1; then
    ssh -o BatchMode=yes "$HOST" "type $LOG"
    exit 0
  fi
  sleep 10
done
echo "still running; last lines:"
ssh -o BatchMode=yes "$HOST" "powershell -NoProfile -Command \"Get-Content '$LOG' -Tail 5\""
exit 3
