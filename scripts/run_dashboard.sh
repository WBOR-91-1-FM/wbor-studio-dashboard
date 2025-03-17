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

SLEEP_AMOUNT_SECS_UPON_PANIC=50
PROJECT_DIR="/Users/wborguest/wbor-studio-dashboard" # Full path to project dir
CRASH_DISCORD_WEBHOOK=`jq -r '.dashboard_crash_discord_webhook_url' ../assets/api_keys.json`

# set -x # Print out all lines that are run
cd "$PROJECT_DIR" || exit # Navigate to the project dir

########## Second, seeing that the project builds

cargo build --release

if [ $? -ne 0 ]; then
	exit 1
fi

########## Third, if the dashboard fails at any point, try a relaunch

while true; do
	echo -e "---------------------------------------\n\nStarting dashboard at $(date)\n" >> "$PROJECT_DIR/project.log"
	RUST_BACKTRACE=1 RUST_LOG=wbor_studio_dashboard cargo run --release >>"$PROJECT_DIR/project.log" 2>&1

	EXIT_CODE=$?

	if [ $EXIT_CODE -eq 0 ]; then
		echo "Dashboard was killed peacefully. Exiting."
		break
	else
		echo "Something went wrong with the dashboard (likely a panic, which should be addressed!). \
Wait for $SLEEP_AMOUNT_SECS_UPON_PANIC seconds, then try a relaunch."

		if [ "$CRASH_DISCORD_WEBHOOK" != "null" ]; then
			escaped_log=$(tail -n 12 "$PROJECT_DIR/project.log" | jq -Rs .)
			send_discord_webhook "$CRASH_DISCORD_WEBHOOK" "\"The dashboard crashed! Here's a bit of the log:\""
			send_discord_webhook "$CRASH_DISCORD_WEBHOOK" "$escaped_log"
		else
			echo "No Discord webhook available for alerting a crash!"
		fi

		sleep $SLEEP_AMOUNT_SECS_UPON_PANIC
	fi
done
