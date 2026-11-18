#!/bin/bash
# Drives the zgx gallery's Providers example through the Cloud key flow,
# snapshotting each step; writes frames/NNN.png and durations.txt.
set -euo pipefail
Z=~/zgx-cli/bin/zgx
OUT=/tmp/claude-swift-onyx/gif/${1:-dark}
mkdir -p "$OUT/frames"
: > "$OUT/durations.txt"
n=0
i() { "$Z" inspect "$@" >/dev/null; }
pause() { i sleep "$1"; }
snap() { # hold_ms
	n=$((n + 1))
	local f
	f=$(printf '%s/frames/%03d.png' "$OUT" "$n")
	i snapshot '#gallery' --out "$f"
	echo "$f $1" >> "$OUT/durations.txt"
}
value() { "$Z" inspect tree 'role:dialog' --depth 8 | grep -E "textbox \"$1\"" | head -1; }

KEY='role:dialog >> role:textbox "API key"'
URL='role:dialog >> role:textbox "Base URL"'

# Start: Cloud, page at its top, no gateway yet.
i set ex_pv_cloud_key_open false
i set ex_pv_gateway null
i set ex_pv_device '"cloud"'
i scroll @252 -3000
pause 500
snap 1600
# Down to Cloud's API keys.
for _ in 1 2 3; do i scroll @252 200; pause 120; snap 110; done
pause 200
snap 900
# The Anthropic key's dialog.
i click '#stage >> role:button "Anthropic API key"'
pause 60
snap 70
pause 250
snap 900
# The key, masked as it's typed.
i click "$KEY"
for chunk in "sk-ant-" "api03-" "demo-0000" "0000000081e5"; do i type "$KEY" "$chunk"; pause 90; snap 150; done
snap 700
# An LLM gateway: its base URL asked, Save waits.
i click 'role:dialog >> role:radio "LLM gateway"'
pause 250
snap 1500
# (`type` clicks the field's middle first, mid-text once it's long: each
# step clears it and types the whole of what it says by then.)
seturl() { i click "$URL"; i key super+a; i key Backspace; i type "$URL" "$1"; pause 90; }
# Plain http: refused as it's typed.
for t in "http" "http://llm-" "http://llm-gateway" "http://llm-gateway.halcyon.dev"; do seturl "$t"; snap 160; done
snap 1900
# Typed again with https.
for t in "https://" "https://llm-gateway" "https://llm-gateway.halcyon.dev"; do seturl "$t"; snap 160; done
value "Base URL" >&2
snap 1700
# /v1 on the end: refused.
seturl "https://llm-gateway.halcyon.dev/v1"
snap 2000
# Off again, then the gateway's path.
seturl "https://llm-gateway.halcyon.dev/"
snap 500
for t in "https://llm-gateway.halcyon.dev/anth" "https://llm-gateway.halcyon.dev/anthropic/"; do seturl "$t"; snap 170; done
value "Base URL" >&2
snap 2000
# Saved: the dialog goes, the card says where its requests go.
i click 'role:dialog >> role:button "Save"'
pause 60
snap 70
pause 250
snap 600
for _ in 1 2 3 4; do i scroll @252 -180; pause 110; snap 110; done
i scroll @252 -3000
pause 200
snap 3000
echo "frames: $n" >&2
