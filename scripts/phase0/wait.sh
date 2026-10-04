#!/usr/bin/env bash
# Wait up to about nine minutes for a desktop-session job, then print its log.
# Exit codes:
#   0  the job finished; its log is printed (the PHASE0-DONE line says status=ok or status=error)
#   3  still running: run this again
#   4  the job is not running and its log has no PHASE0-DONE line (it never started, or was killed)
#   5  the mini PC cannot be reached over SSH
set -uo pipefail
[ "$#" -eq 1 ] || { echo "usage: scripts/phase0/wait.sh <log name>" >&2; exit 2; }
HOST="${LAZYMKV_BOX:-USER@makemkv-box}"
LOG="C:\\lazymkv\\phase0\\logs\\$1.txt"
deadline=$((SECONDS + 520))
unreachable=0
idle=0

finished() { ssh -o BatchMode=yes "$HOST" "findstr /b PHASE0-DONE $LOG" >/dev/null 2>&1; }
print_log() { ssh -o BatchMode=yes "$HOST" "type $LOG"; }

while :; do
  finished
  rc=$?
  if [ "$rc" -eq 0 ]; then
    print_log
    exit 0
  fi
  if [ "$rc" -eq 255 ]; then
    unreachable=$((unreachable + 1))
    if [ "$unreachable" -ge 3 ]; then
      echo "cannot reach $HOST over SSH" >&2
      exit 5
    fi
  else
    unreachable=0
    state="$(ssh -o BatchMode=yes "$HOST" "powershell -NoProfile -Command \"(Get-ScheduledTask -TaskName lazymkv-phase0 -ErrorAction SilentlyContinue).State\"" 2>/dev/null | tr -d '\r\n ')"
    if [ "$state" = "Running" ]; then
      idle=0
    else
      idle=$((idle + 1))
    fi
    # Two idle observations in a row, so a job that is just starting or just finishing is not misjudged.
    if [ "$idle" -ge 2 ]; then
      if finished; then
        print_log
        exit 0
      fi
      echo "the job is not running (task state: ${state:-unknown}) and $1.txt has no PHASE0-DONE line; last lines, if any:" >&2
      ssh -o BatchMode=yes "$HOST" "powershell -NoProfile -Command \"Get-Content '$LOG' -Tail 20 -ErrorAction SilentlyContinue\""
      exit 4
    fi
  fi
  [ "$SECONDS" -ge "$deadline" ] && break
  sleep 10
done
echo "still running; last lines:"
ssh -o BatchMode=yes "$HOST" "powershell -NoProfile -Command \"Get-Content '$LOG' -Tail 5\""
exit 3
