#!/bin/bash
LINE='error[E0425]: cannot find value `notvalid` in this scope'

if echo "$LINE" | grep -q "error\[E"; then
    echo "Match found!"
    
    # Extract code
    if [[ "$LINE" =~ error\[E([0-9]+)\]: ]]; then
        CODE="${BASH_REMATCH[1]}"
        echo "Code: $CODE"
    fi
    
    # Extract message (after "]: ")
    MESSAGE="${LINE#*]: }"
    echo "Message: $MESSAGE"
fi
