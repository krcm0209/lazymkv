#!/usr/bin/env bash
# Wait up to about nine minutes for a desktop-session job, then print its log.
# Exit codes:
#   0  the job finished and its script did not throw (PHASE0-DONE status=ok); the log is printed
#   1  the job finished but its script threw (PHASE0-DONE status=error); the log is printed
#   3  no PHASE0-DONE line yet: run this again
#   4  the job is not running and its log has no PHASE0-DONE line (it never started, or was killed)
#   5  the mini PC cannot be reached over SSH
set -uo pipefail
[ "$#" -eq 1 ] || { echo "usage: scripts/phase0/wait.sh <log name>" >&2; exit 2; }
HOST="${LAZYMKV_BOX:-USER@makemkv-box}"
LOG="C:\\lazymkv\\phase0\\logs\\$1.txt"
SSH=(ssh -o BatchMode=yes -o ConnectTimeout=10)
deadline=$((SECONDS + 520))
unreachable=0
idle=0
state=""

finished() { "${SSH[@]}" "$HOST" "findstr /b PHASE0-DONE $LOG" >/dev/null 2>&1; }

print_log_and_exit() {
  local out
  out="$("${SSH[@]}" "$HOST" "type $LOG")" || { echo "the job finished but its log could not be fetched" >&2; exit 5; }
  printf '%s\n' "$out"
  case "$out" in *"PHASE0-DONE status=error"*) exit 1 ;; esac
  exit 0
}

while :; do
  finished
  rc=$?
  [ "$rc" -eq 0 ] && print_log_and_exit
  if [ "$rc" -eq 255 ]; then
    unreachable=$((unreachable + 1))
    if [ "$unreachable" -ge 3 ]; then
      echo "cannot reach $HOST over SSH" >&2
      exit 5
    fi
  else
    unreachable=0
    # Prints the task's state, or None when no job was ever started. Empty output means the question failed.
    state="$("${SSH[@]}" "$HOST" "powershell -NoProfile -Command \"\$t = Get-ScheduledTask -TaskName lazymkv-phase0 -ErrorAction SilentlyContinue; if (\$t) { \$t.State } else { 'None' }\"" 2>/dev/null | tr -d '\r\n ')"
    case "$state" in
      Running|Queued) idle=0 ;;
      Ready|Disabled|None) idle=$((idle + 1)) ;;
      *) ;; # the state could not be read: neither running nor idle
    esac
    # Two idle observations in a row, so a job that is just starting or just finishing is not misjudged.
    if [ "$idle" -ge 2 ]; then
      finished && print_log_and_exit
      echo "the job is not running (task state: $state) and $1.txt has no PHASE0-DONE line; last lines, if any:" >&2
      "${SSH[@]}" "$HOST" "powershell -NoProfile -Command \"Get-Content '$LOG' -Tail 20 -ErrorAction SilentlyContinue\""
      exit 4
    fi
  fi
  [ "$SECONDS" -ge "$deadline" ] && break
  sleep 10
done
echo "no PHASE0-DONE line yet (task state: ${state:-unknown}); last lines:"
"${SSH[@]}" "$HOST" "powershell -NoProfile -Command \"Get-Content '$LOG' -Tail 5 -ErrorAction SilentlyContinue\""
exit 3
