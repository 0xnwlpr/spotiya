#!/bin/sh
set -eu
umask 077
SPOTIFY_DIR=/data/opt/spotify-connect
LOCKDIR=/run/spotify-connect-boot.lock
mkdir "$LOCKDIR" 2>/dev/null || exit 0
trap 'rmdir "$LOCKDIR" 2>/dev/null || true' EXIT
SPOTIFY_ATTEMPT=0
while [ "$SPOTIFY_ATTEMPT" -lt 120 ]; do
    [ ! -e /run/spotify-connect-stop ] || exit 0
    if ip -4 addr show scope global 2>/dev/null | grep -q 'inet '; then
        "$SPOTIFY_DIR/start.sh"
        exit 0
    fi
    SPOTIFY_ATTEMPT=$((SPOTIFY_ATTEMPT + 1))
    sleep 1
done
echo "Spotify Connect: no network address after 120 seconds" >&2
exit 1
