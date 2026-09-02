#!/bin/sh

FUNKEY_IROH_CONFIG=${FUNKEY_IROH_CONFIG:-/etc/default/funkey-iroh}
[ -r "$FUNKEY_IROH_CONFIG" ] && . "$FUNKEY_IROH_CONFIG"

FUNKEY_IROH_DATA_ROOT=${FUNKEY_IROH_DATA_ROOT:-/mnt}
FUNKEY_IROH_STATE_DIR=${FUNKEY_IROH_STATE_DIR:-$FUNKEY_IROH_DATA_ROOT/.funkey-iroh}
FUNKEY_IROH_INBOX=${FUNKEY_IROH_INBOX:-$FUNKEY_IROH_STATE_DIR/inbox}
FUNKEY_IROH_PUBLIC_DIR=${FUNKEY_IROH_PUBLIC_DIR:-$FUNKEY_IROH_DATA_ROOT/FunKey/Iroh}
FUNKEY_IROH_DAEMON=${FUNKEY_IROH_DAEMON:-/usr/bin/funkey-iroh}
FUNKEY_IROH_SERVICE=${FUNKEY_IROH_SERVICE:-/usr/bin/funkey-iroh-service}
FUNKEY_IROH_SESSION=${FUNKEY_IROH_SESSION:-/usr/bin/funkey-iroh-session}
FUNKEY_IROH_SYNC_PEERS=${FUNKEY_IROH_SYNC_PEERS:-$FUNKEY_IROH_STATE_DIR/sync-peers}
FUNKEY_IROH_OUTBOX=${FUNKEY_IROH_OUTBOX:-$FUNKEY_IROH_STATE_DIR/outbox}
FUNKEY_IROH_GAME_DB=${FUNKEY_IROH_GAME_DB:-$FUNKEY_IROH_STATE_DIR/games}
FUNKEY_IROH_BASELINES=${FUNKEY_IROH_BASELINES:-$FUNKEY_IROH_STATE_DIR/baselines}
FUNKEY_IROH_ARCHIVE=${FUNKEY_IROH_ARCHIVE:-$FUNKEY_IROH_STATE_DIR/archive}
FUNKEY_IROH_BACKUPS=${FUNKEY_IROH_BACKUPS:-$FUNKEY_IROH_STATE_DIR/backups}
FUNKEY_IROH_RUNTIME_DIR=${FUNKEY_IROH_RUNTIME_DIR:-/var/run/funkey-iroh}
FUNKEY_IROH_ENDPOINT_LOCK=${FUNKEY_IROH_ENDPOINT_LOCK:-/var/lock/funkey-iroh-endpoint.lock}
FUNKEY_IROH_EXCLUSIVE=${FUNKEY_IROH_EXCLUSIVE:-$FUNKEY_IROH_RUNTIME_DIR/exclusive}

iroh_die()
{
    code=$1
    shift
    printf 'funkey-iroh: %s\n' "$*" >&2
    exit "$code"
}

iroh_warn()
{
    printf 'funkey-iroh: %s\n' "$*" >&2
}

iroh_data_is_mounted()
{
    [ "${FUNKEY_IROH_ASSUME_MOUNTED:-0}" = 1 ] && return 0
    grep -q '[[:space:]]/mnt[[:space:]]' /proc/mounts 2>/dev/null
}

iroh_ensure_state()
{
    mkdir -p \
        "$FUNKEY_IROH_STATE_DIR" \
        "$FUNKEY_IROH_INBOX" \
        "$FUNKEY_IROH_OUTBOX" \
        "$FUNKEY_IROH_GAME_DB" \
        "$FUNKEY_IROH_BASELINES" \
        "$FUNKEY_IROH_ARCHIVE" \
        "$FUNKEY_IROH_BACKUPS" \
        "$FUNKEY_IROH_RUNTIME_DIR"
}

iroh_valid_name()
{
    case "${1:-}" in
        ''|*[!A-Za-z0-9._-]*|.*|*-|*_) return 1 ;;
        *) [ "${#1}" -le 64 ] ;;
    esac
}

iroh_sanitize()
{
    # Keep this aligned with the Rust receiver's path-component policy.
    printf '%s' "${1:-unnamed}" |
        tr '\t\r\n /\\:;|?*"<>' '_________________' |
        sed -e 's/[^A-Za-z0-9._-]/-/g' \
            -e 's/[-_][-_]*/_/g' \
            -e 's/^[._-]*//' \
            -e 's/[._-]*$//' |
        cut -c1-96
}

iroh_safe_path()
{
    case "${1:-}" in
        *'
'*|*'	'*) return 1 ;;
        "$FUNKEY_IROH_DATA_ROOT"/*) ;;
        *) return 1 ;;
    esac
    case "/${1#/}/" in
        */../*|*/./*) return 1 ;;
    esac
    return 0
}

iroh_canonical_existing()
{
    readlink -f "$1" 2>/dev/null
}

iroh_path_under()
{
    candidate=$(iroh_canonical_existing "$1") || return 1
    parent=$(iroh_canonical_existing "$2") || return 1
    case "$candidate" in
        "$parent"|"$parent"/*) printf '%s\n' "$candidate" ;;
        *) return 1 ;;
    esac
}

iroh_hash_file()
{
    path=$1
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$path" | awk '{print $1}'
    else
        cksum "$path" | awk '{print $1 ":" $2}'
    fi
}

iroh_atomic_write()
{
    destination=$1
    temporary="${destination}.tmp.$$"
    umask 077
    cat > "$temporary" || {
        rm -f "$temporary"
        return 1
    }
    sync "$temporary" 2>/dev/null || sync
    mv "$temporary" "$destination"
}

iroh_line_present()
{
    needle=$1
    file=$2
    [ -r "$file" ] && grep -Fxq "$needle" "$file"
}

iroh_add_line()
{
    value=$1
    file=$2
    mkdir -p "$(dirname "$file")"
    if ! iroh_line_present "$value" "$file"; then
        printf '%s\n' "$value" >> "$file"
        sync "$file" 2>/dev/null || :
    fi
}

iroh_remove_line()
{
    value=$1
    file=$2
    [ -e "$file" ] || return 0
    temporary="${file}.tmp.$$"
    awk -v value="$value" '$0 != value' "$file" > "$temporary" &&
        mv "$temporary" "$file"
}

iroh_peer_exists()
{
    name=$1
    "$FUNKEY_IROH_DAEMON" peer list 2>/dev/null |
        awk -F '\t' -v wanted="$name" '$1 == wanted { found=1 } END { exit !found }'
}

iroh_baseline_path()
{
    peer=$(iroh_sanitize "$1")
    system=$(iroh_sanitize "$2")
    game=$(iroh_sanitize "$3")
    filename=$(iroh_sanitize "$4")
    printf '%s/%s/%s/%s/%s.sha256\n' \
        "$FUNKEY_IROH_BASELINES" "$peer" "$system" "$game" "$filename"
}

iroh_write_baseline()
{
    peer=$1
    system=$2
    game=$3
    file=$4
    hash=$5
    baseline=$(iroh_baseline_path "$peer" "$system" "$game" "$(basename "$file")")
    mkdir -p "$(dirname "$baseline")"
    printf '%s\n' "$hash" | iroh_atomic_write "$baseline"
}

iroh_read_baseline()
{
    baseline=$(iroh_baseline_path "$1" "$2" "$3" "$4")
    [ -r "$baseline" ] && sed -n '1p' "$baseline"
}

iroh_notify()
{
    seconds=$1
    shift
    if command -v notif >/dev/null 2>&1; then
        notif display "$seconds" "$*" >/dev/null 2>&1 || :
    fi
}
