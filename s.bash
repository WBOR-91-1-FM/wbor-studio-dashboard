auth_details=`jq -r ".twilio_account_sid, .twilio_auth_token" assets/api_keys.json`
read account_sid auth_token <<< $auth_details

##########

deletion_count=200

base_request_url="https://api.twilio.com"
request_url="$base_request_url/2010-04-01/Accounts/$account_sid/Messages.json?PageSize=$deletion_count"
msgs_json=`curl -s -X GET $request_url -u $account_sid:$auth_token`

# TODO: how do I pass many messages to delete at once?

for ((i=0; i<$deletion_count; i++))
do
	msg_fields=`jq -r ".messages[$i] | .uri, .body, .from, .date_sent" <<< $msgs_json`
	read uri body from date_sent <<< $msg_fields
	curl -X DELETE "$base_request_url/$uri" -u $account_sid:$auth_token
	echo "Deleted message: '$body' (from '$from', on '$date_sent')\n---"
done
