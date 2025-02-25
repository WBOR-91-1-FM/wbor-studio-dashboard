#!/bin/bash

########## First, do some initial setup

SLEEP_AMOUNT_SECS_UPON_PANIC=50
PROJECT_DIR="/Users/wborguest/wbor-studio-dashboard" # Full path to project dir

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
	RUST_LOG=wbor_studio_dashboard cargo run --release >>"$PROJECT_DIR/project.log" 2>&1

	EXIT_CODE=$?

	if [ $EXIT_CODE -eq 0 ]; then
		echo "Dashboard was killed peacefully. Exiting."
		break
	else
		echo "Something went wrong with the dashboard (likely a panic, which should be addressed!). \
Wait for $SLEEP_AMOUNT_SECS_UPON_PANIC seconds, then try a relaunch."

		sleep $SLEEP_AMOUNT_SECS_UPON_PANIC
	fi
done
