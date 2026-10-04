#!/usr/bin/env bash
# Start a phase 0 script in the owner's desktop session on the mini PC.
# Usage: scripts/phase0/job.sh <script.ps1> <log name> [script arguments, without quotes]
set -euo pipefail
HOST="${LAZYMKV_BOX:-USER@makemkv-box}"
SCRIPT="$1"
LOG="$2"
shift 2
ssh -o BatchMode=yes "$HOST" "powershell -NoProfile -ExecutionPolicy Bypass -Command \"& C:\\lazymkv\\src\\scripts\\phase0\\run-interactive.ps1 -Script $SCRIPT -ArgLine '$*' -Log C:\\lazymkv\\phase0\\logs\\$LOG.txt\""
