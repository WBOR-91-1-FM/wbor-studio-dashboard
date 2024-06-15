#!/bin/bash

# Script param: surprise name.

fail() {
	echo "$1"
	exit 1
}

path="$1"

echo "Note: if the dashboard is not in focus, this script may hang until it becomes so."

if [[ "$path" == "" ]]; then
	fail "Please provide a surprise path (in the format of \"assets/<surprise_name>\")!"
fi

printf "$path" | nc -U /tmp/surprises_wbor_studio_dashboard.sock || fail "Could not send the path to the dashboard's socket!"

echo ">>> Sent the surprise to the dashboard. Check the dashboard logs to see that the surprise was received."
