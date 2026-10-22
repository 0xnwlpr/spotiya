#!/bin/sh
set -eu
umask 077
SPOTIFY_DIR=/data/opt/spotify-connect
SPOTIFY_EXE=$(readlink -f "$SPOTIFY_DIR/spotiya")
PIDFILE=/run/spotify-connect.pid
if [ -f "$PIDFILE" ]; then
    SPOTIFY_PID=$(cat "$PIDFILE")
    case "$SPOTIFY_PID" in
        ''|*[!0-9]*) ;;
        *)
            if [ "$(readlink "/proc/$SPOTIFY_PID/exe" 2>/dev/null || true)" = "$SPOTIFY_EXE" ]; then
                echo "Spotify Connect already running (pid $SPOTIFY_PID)"
                exit 0
            fi
            ;;
    esac
fi
nohup setsid "$SPOTIFY_DIR/run.sh" </dev/null >/tmp/spotify-connect.log 2>&1 &
SPOTIFY_PID=$!
echo "$SPOTIFY_PID" > "$PIDFILE"
sleep 1
kill -0 "$SPOTIFY_PID"
echo "Spotify Connect started (pid $SPOTIFY_PID)"
