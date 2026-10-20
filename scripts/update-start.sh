#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
BIN_PATH="${ROOT_DIR}/server/target/release/iowebsite"
RUN_DIR="${ROOT_DIR}/.run"
LOG_FILE="${RUN_DIR}/iowebsite.log"
PID_FILE="${RUN_DIR}/iowebsite.pid"
LOCK_DIR="${RUN_DIR}/update.lock.d"
PORT="${PORT:-2993}"
STATIC_DIR="${STATIC_DIR:-${ROOT_DIR}/dist}"
MODE="background"
SKIP_UPDATE=0

usage() {
    cat <<'USAGE'
用法：
  scripts/update-start.sh [--foreground|--background] [--skip-update]

选项：
  --foreground   更新构建后以前台方式启动（适合 systemd、调试和容器）
  --background   更新构建后在后台启动（默认，写入 .run/ 下的 PID 和日志）
  --skip-update  跳过 git pull（仍执行 npm 与 Rust 构建）
  -h, --help     显示帮助

环境变量：
  PORT          监听端口，默认 2993
  STATIC_DIR    前端 dist 目录，默认仓库根目录/dist

安全说明：
  本脚本不会调用 sudo，也不会 kill 未由本仓库目标二进制启动的进程。
USAGE
}

while (($# > 0)); do
    case "$1" in
        --foreground)
            MODE="foreground"
            ;;
        --background)
            MODE="background"
            ;;
        --skip-update)
            SKIP_UPDATE=1
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            printf '未知参数：%s\n\n' "$1" >&2
            usage >&2
            exit 64
            ;;
    esac
    shift
done

if [[ ! "$PORT" =~ ^[0-9]+$ ]] || ((PORT < 1 || PORT > 65535)); then
    printf 'PORT 必须是 1..65535，当前值：%s\n' "$PORT" >&2
    exit 64
fi

mkdir -p "$RUN_DIR"
if ! mkdir "$LOCK_DIR" 2>/dev/null; then
    printf '检测到更新锁：%s\n如确认没有更新任务在运行，可人工删除该目录。\n' "$LOCK_DIR" >&2
    exit 75
fi

cleanup() {
    rmdir "$LOCK_DIR" 2>/dev/null || true
}
trap cleanup EXIT
trap 'printf "命令在第 %s 行失败（退出码 %s）\n" "$LINENO" "$?" >&2' ERR

require_command() {
    if ! command -v "$1" >/dev/null 2>&1; then
        printf '缺少命令：%s\n' "$1" >&2
        exit 69
    fi
}

process_is_ours() {
    local pid="$1"
    local expected="$2"
    local actual=""

    [[ -r "/proc/${pid}/exe" ]] || return 1
    actual="$(readlink -f "/proc/${pid}/exe" 2>/dev/null || true)"
    if [[ -z "$actual" ]]; then
        return 1
    fi

    # cargo 原地替换二进制时，旧 /proc/PID/exe 可能带有 " (deleted)"。
    [[ "$actual" == "$expected" || "$actual" == "${expected} (deleted)" ]]
}

stop_old_process() {
    local pid=""
    local waited=0

    if [[ ! -f "$PID_FILE" ]]; then
        printf '未发现旧进程 PID 文件，跳过停止。\n'
        return 0
    fi

    pid="$(tr -cd '0-9' < "$PID_FILE" || true)"
    if [[ -z "$pid" ]]; then
        printf 'PID 文件无效，删除：%s\n' "$PID_FILE"
        rm -f "$PID_FILE"
        return 0
    fi

    if ! kill -0 "$pid" 2>/dev/null; then
        printf '旧 PID %s 已不存在，清理 PID 文件。\n' "$pid"
        rm -f "$PID_FILE"
        return 0
    fi

    if ! process_is_ours "$pid" "$BIN_PATH"; then
        printf '拒绝停止 PID %s：它不是本仓库的 %s，可能已被其他进程复用。\n' \
            "$pid" "$BIN_PATH" >&2
        printf '请人工确认进程后删除 %s，再重试。\n' "$PID_FILE" >&2
        return 1
    fi

    printf '安全停止旧进程 PID %s...\n' "$pid"
    kill -TERM "$pid" 2>/dev/null || true
    while kill -0 "$pid" 2>/dev/null && ((waited < 100)); do
        sleep 0.1
        waited=$((waited + 1))
    done

    if kill -0 "$pid" 2>/dev/null; then
        if ! process_is_ours "$pid" "$BIN_PATH"; then
            printf '等待期间 PID %s 身份发生变化，拒绝强制停止。\n' "$pid" >&2
            return 1
        fi
        printf '进程 10 秒内未退出，发送 KILL...\n'
        kill -KILL "$pid" 2>/dev/null || true
        sleep 0.2
    fi

    rm -f "$PID_FILE"
    printf '旧进程已停止。\n'
}

wait_until_ready() {
    local attempt=0
    while ((attempt < 50)); do
        if (exec 3<>"/dev/tcp/127.0.0.1/${PORT}") 2>/dev/null; then
            exec 3>&- 3<&- || true
            return 0
        fi
        sleep 0.1
        attempt=$((attempt + 1))
    done
    return 1
}

start_background() {
    printf '后台启动：%s（PORT=%s）\n' "$BIN_PATH" "$PORT"
    (
        cd "$ROOT_DIR"
        nohup env PORT="$PORT" STATIC_DIR="$STATIC_DIR" "$BIN_PATH" \
            >>"$LOG_FILE" 2>&1 </dev/null &
        printf '%s\n' "$!" >"$PID_FILE"
    )

    if ! wait_until_ready; then
        printf '服务在 5 秒内未监听 %s，最近日志：\n' "$PORT" >&2
        tail -n 40 "$LOG_FILE" >&2 || true
        return 1
    fi

    printf '服务已就绪：%s\n' "$(tr -cd '0-9' < "$PID_FILE")"
    printf '日志：%s\n' "$LOG_FILE"
}

require_command npm
require_command cargo
[[ -f "$ROOT_DIR/package.json" ]] || {
    printf '缺少 package.json：%s\n' "$ROOT_DIR/package.json" >&2
    exit 66
}
[[ -f "$ROOT_DIR/VERSION" ]] || {
    printf '缺少 VERSION：%s\n' "$ROOT_DIR/VERSION" >&2
    exit 66
}

if ((SKIP_UPDATE == 0)); then
    require_command git
    printf '更新代码（fast-forward）...\n'
    git -C "$ROOT_DIR" pull --ff-only
fi

printf '安装前端依赖...\n'
(
    cd "$ROOT_DIR"
    npm install
)

printf '构建前端...\n'
(
    cd "$ROOT_DIR"
    npm run build
)

printf '构建 Rust release...\n'
cargo build --manifest-path "${ROOT_DIR}/server/Cargo.toml" --release

[[ -x "$BIN_PATH" ]] || {
    printf '构建后找不到可执行文件：%s\n' "$BIN_PATH" >&2
    exit 66
}

# 构建全部成功后才停止旧进程，避免构建失败导致不必要停服。
stop_old_process

VERSION="$(tr -d '\r\n' < "$ROOT_DIR/VERSION")"
printf '版本：%s\n' "$VERSION"

if [[ "$MODE" == "foreground" ]]; then
    printf '前台启动：%s（PORT=%s）\n' "$BIN_PATH" "$PORT"
    exec env PORT="$PORT" STATIC_DIR="$STATIC_DIR" "$BIN_PATH"
fi

start_background
