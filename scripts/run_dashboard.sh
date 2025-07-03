#!/bin/bash

# Arguments: the webhook, and the escaped content to send
send_discord_webhook() {
	webhook="$1"
	escaped_content="$2"

	curl -H "Content-Type: application/json" \
		-X POST \
		-d "{\"content\": $escaped_content}" \
		"$webhook"
}

########## First, do some initial setup

NUM_PROJECT_LOG_LINES=20
SLEEP_AMOUNT_SECS_UPON_PANIC=50
PROJECT_DIR="/Users/wborguest/wbor-studio-dashboard" # Full path to project dir

cd "$PROJECT_DIR" || exit # Navigate to the project dir

# Setting this after navigating to the project dir, since we need to be in the right directory for it to work
CRASH_DISCORD_WEBHOOK=`jq -r '.api_keys.dashboard_crash_discord_webhook_url' assets/env.json`

########## Second, seeing that the project builds

cargo build --release

if [ $? -ne 0 ]; then
	exit 1
fi

########## Third, if the dashboard fails at any point, try a relaunch

print_to_log() {
	echo -e "$1" >> "$PROJECT_DIR/project.log"
}

while true; do
	print_to_log "---------------------------------------\n\nStarting dashboard at $(date)\n"
	RUST_BACKTRACE=1 RUST_LOG=wbor_studio_dashboard cargo run --release >>"$PROJECT_DIR/project.log" 2>&1

	EXIT_CODE=$?

	if [ $EXIT_CODE -eq 0 ]; then
		print_to_log "\nDashboard was killed peacefully. Exiting.\n"
		break
	else
		print_to_log "\nSomething went wrong with the dashboard (likely a panic, which should be addressed!). \
Wait for $SLEEP_AMOUNT_SECS_UPON_PANIC seconds, then try a relaunch.\n"

		if [ "$CRASH_DISCORD_WEBHOOK" != "null" ]; then
			log_tail=$(tail -n $NUM_PROJECT_LOG_LINES "$PROJECT_DIR/project.log")
			escaped_log=$(printf '```\n%s\n```' "$log_tail" | jq -Rs .)
			curr_time=$(date)

			send_discord_webhook "$CRASH_DISCORD_WEBHOOK" "\"$curr_time\nThe dashboard crashed! Here's a bit of the log:\""
			send_discord_webhook "$CRASH_DISCORD_WEBHOOK" "$escaped_log"
		else
			print_to_log "\nNo Discord webhook available for alerting a crash!\n"
		fi

		sleep $SLEEP_AMOUNT_SECS_UPON_PANIC
	fi
done
