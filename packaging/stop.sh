#!/bin/sh
set -eu
PIDFILE=/run/spotify-connect.pid
SPOTIFY_DIR=/data/opt/spotify-connect
SPOTIFY_EXE=$(readlink -f "$SPOTIFY_DIR/spotiya")
if [ ! -f "$PIDFILE" ]; then
    echo "Spotify Connect is not running"
    exit 0
fi
SPOTIFY_PID=$(cat "$PIDFILE")
case "$SPOTIFY_PID" in
    ''|*[!0-9]*) echo "Invalid pid file" >&2; exit 1 ;;
esac
if [ "$(readlink "/proc/$SPOTIFY_PID/exe" 2>/dev/null || true)" = "$SPOTIFY_EXE" ]; then
    kill -TERM "$SPOTIFY_PID"
    SPOTIFY_WAIT=0
    while [ "$(readlink "/proc/$SPOTIFY_PID/exe" 2>/dev/null || true)" = "$SPOTIFY_EXE" ]; do
        if [ "$SPOTIFY_WAIT" -ge 50 ]; then
            echo "Spotify Connect is still stopping" >&2
            exit 1
        fi
        sleep 0.1
        SPOTIFY_WAIT=$((SPOTIFY_WAIT + 1))
    done
fi
rm -f "$PIDFILE"
echo "Spotify Connect stopped"
