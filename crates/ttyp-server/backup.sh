#!/bin/sh
# Daily SQLite backup for the ttyp-backup sidecar.
#
#   ttyp-backup          loop: back up at 00:30 UTC every day
#   ttyp-backup once     back up now and exit (also used to test restores)
#
# Uses SQLite's online backup (never a plain copy: with WAL that can tear),
# checks the copy, gzips it into $TTYP_BACKUP_DIR and keeps the newest
# $TTYP_BACKUP_KEEP files.
set -eu

DB="${TTYP_DB:-/data/ttyp.db}"
DIR="${TTYP_BACKUP_DIR:-/backups}"
KEEP="${TTYP_BACKUP_KEEP:-14}"
AT="${TTYP_BACKUP_AT:-00:30}"   # HH:MM UTC

backup() {
    stamp=$(date -u +%Y-%m-%d)
    out="$DIR/ttyp-$stamp.db"
    if [ ! -f "$DB" ]; then
        echo "backup: $DB does not exist yet, skipping"
        return 0
    fi
    rm -f "$out" "$out.gz"
    sqlite3 "$DB" ".backup '$out'"
    check=$(sqlite3 "$out" "PRAGMA integrity_check;")
    if [ "$check" != "ok" ]; then
        echo "backup: integrity check FAILED for $out: $check" >&2
        rm -f "$out"
        return 1
    fi
    gzip -f "$out"
    echo "backup: wrote $out.gz ($(du -h "$out.gz" | cut -f1)), integrity ok"
    # Prune: newest $KEEP by name (dates sort), the rest go.
    ls -1 "$DIR"/ttyp-*.db.gz 2>/dev/null | sort -r | tail -n +"$((KEEP + 1))" | while read -r old; do
        rm -f "$old"
        echo "backup: pruned $old"
    done
}

seconds_until() {
    # Seconds until the next $1 (HH:MM) UTC.
    now=$(date -u +%s)
    target=$(date -u -d "$(date -u +%Y-%m-%d) $1" +%s)
    if [ "$target" -le "$now" ]; then
        target=$((target + 86400))
    fi
    echo $((target - now))
}

case "${1:-loop}" in
    once) backup ;;
    loop)
        while true; do
            wait=$(seconds_until "$AT")
            echo "backup: next run in ${wait}s (at $AT UTC)"
            sleep "$wait"
            backup || echo "backup: failed, will retry tomorrow" >&2
        done
        ;;
    *) echo "usage: ttyp-backup [once|loop]" >&2; exit 2 ;;
esac
