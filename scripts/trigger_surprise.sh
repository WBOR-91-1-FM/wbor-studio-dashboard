#!/bin/bash

# Script param: surprise name.

log() {
	echo ">>> $1"
}

fail() {
	log "$1"
	exit 1
}

path="$1"

log "Note: if the dashboard is not in focus, this script may hang until it becomes so."

if [[ "$path" == "" ]]; then
	fail "Please provide a surprise path (in the format of \"assets/<surprise_name>\")!"
fi

printf "$path" | nc -U /tmp/wbor_studio_dashboard_surprises.sock || fail "Could not send the path to the dashboard's socket!"

log "Sent the surprise to the dashboard. Check the dashboard logs to see that the surprise was received."
