#!/bin/bash

##########

log() {
	echo ">>> $1"
}

fail() {
	log "$1"
	exit 1
}

##########

send_to_socket() {
	log "Note: when sending a message to a socket, if the dashboard is not in focus, this script may hang until it becomes so."

	socket_name="$1"
	msg="$2"

	socket_path="/tmp/wbor_studio_dashboard_$socket_name.sock"
	printf "$msg" | nc -U "$socket_path" || fail "Could not send the message to this dashboard socket: '$socket_path'. Make sure that the dashboard is running."
}

send_surprise() {
	path="$1"

	if [[ "$path" == "" ]]; then
		fail "Please provide a surprise path as the second argument (in the format of \"assets/<surprise_name>\")!"
	fi

	send_to_socket "surprises" "$path"
	log "Sent the surprise to the dashboard. Check the dashboard logs to see that the surprise was received."
}

##########

communication_type="$1"

case "$communication_type" in
	"surprise")
		send_surprise "$2"
		;;
	"spinitron_refresh")
		send_to_socket "spinitron_instant_update" "" # Sending an empty message
		;;
	"twilio_refresh")
		send_to_socket "twilio_instant_update" "" # Sending an empty message
		;;
	*)
		fail "Please provide a valid communication type as the first argument ('surprise', 'spinitron_refresh', or 'twilio_refresh')."
		;;
esac
