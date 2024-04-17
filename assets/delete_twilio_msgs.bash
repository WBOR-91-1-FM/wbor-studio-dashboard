auth_details=`jq -r ".twilio_account_sid, .twilio_auth_token" api_keys.json`
read account_sid auth_token <<< $auth_details

##########

deletion_count=200

base_request_url="https://api.twilio.com"
request_url="$base_request_url/2010-04-01/Accounts/$account_sid/Messages.json?PageSize=$deletion_count"
msgs_json=`curl -s -X GET $request_url -u $account_sid:$auth_token`

# TODO: how do I pass many messages to delete at once?

for ((i=0; i<$deletion_count; i++)) do
	# TODO: how to read these 4 with just 1 `jq` command?
	message_info=`jq -r ".messages[$i]" <<< $msgs_json`
	read uri from <<< `jq -r ".uri, .from" <<< $message_info`
	body=`jq -r ".body" <<< $message_info`
	date_sent=`jq -r ".date_sent" <<< $message_info`

	curl -X DELETE "$base_request_url/$uri" -u $account_sid:$auth_token &
	echo "Deleted message: '$body' (from '$from', on '$date_sent')\n---"
done
