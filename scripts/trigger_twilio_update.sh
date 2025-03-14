#!/bin/bash

log() {
	echo ">>> $1"
}

fail() {
	log "$1"
	exit 1
}

log "Note: if the dashboard is not in focus, this script may hang until it becomes so."

# Echoing any character to send to the socket (arbitrary)
printf "a" | nc -U /tmp/wbor_studio_dashboard_twilio_instant_update.sock || fail "Could use the Twilio instant update socket!"
