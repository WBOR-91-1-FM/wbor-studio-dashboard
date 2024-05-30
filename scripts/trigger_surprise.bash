#!/bin/bash

# Script param: surprise index.
# Note: this script will kill the dashboard if it hasn't been fully launched yet!
# So, wait for it to fully launch before running it.

##########

# Param: message
fail() {
	echo $1
	exit 1
}

surprise_index=$1

unsigned_int_pattern='^[0-9]+$'
[[ $surprise_index =~ $unsigned_int_pattern ]] || fail "No valid surprise index supplied."

##########

# Param: the signal to send
try_signal() {
	pkill -f -$1 wbor-studio-dashboard || fail "Could not send the signal '$1' to the dashboard (check if it's running)!"
}

# 1. Send repeated signals to increment surprise index (SIGUSR1)
for ((i = 0; i < $surprise_index; i++)); do try_signal SIGUSR1; done

# 2. Send signal to trigger surprise and reset surprise indexw (SIGUSR2)
try_signal SIGUSR2
