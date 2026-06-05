#!/usr/bin/env bash
set -euo pipefail

fail=0

check_max_lines() {
  local path=$1
  local max_lines=$2

  if [[ ! -f "$path" ]]; then
    printf 'missing tracked hotspot: %s\n' "$path" >&2
    fail=1
    return
  fi

  local lines
  lines=$(wc -l <"$path")
  lines=${lines//[[:space:]]/}

  if ((lines > max_lines)); then
    printf 'too large: %s has %s lines, max %s\n' "$path" "$lines" "$max_lines" >&2
    fail=1
  else
    printf 'ok: %s has %s/%s lines\n' "$path" "$lines" "$max_lines"
  fi
}

check_no_complexity_allows() {
  local path=$1
  local pattern='#!?\[(allow|expect)\(clippy::(too_many_lines|cognitive_complexity)'

  if [[ ! -e "$path" ]]; then
    printf 'missing complexity hotspot: %s\n' "$path" >&2
    fail=1
    return
  fi

  if command -v rg >/dev/null 2>&1; then
    if rg -n "$pattern" "$path"; then
      printf 'complexity suppression found in maintainability hotspot: %s\n' "$path" >&2
      fail=1
    fi
  elif grep -RInE "$pattern" "$path"; then
    printf 'complexity suppression found in maintainability hotspot: %s\n' "$path" >&2
    fail=1
  fi
}

check_max_lines crates/unifly/src/tui/widgets/hyperchart/time_series.rs 550
check_max_lines crates/unifly/src/tui/widgets/hyperchart/pixel.rs 350
check_max_lines crates/unifly/src/tui/widgets/hyperchart/canvas.rs 425
check_max_lines crates/unifly/src/tui/widgets/hyperchart/raster.rs 400
check_max_lines crates/unifly/src/tui/widgets/hyperchart/scene.rs 260
check_max_lines crates/unifly/src/tui/widgets/hyperchart/model.rs 180
check_max_lines crates/unifly/src/tui/render_caps.rs 350
check_max_lines crates/unifly/src/tui/render_scheduler.rs 80

check_max_lines crates/unifly/src/cli/commands/vpn.rs 750
check_max_lines crates/unifly/src/cli/commands/vpn/render.rs 700
check_max_lines crates/unifly-api/src/command/requests/policy.rs 800
check_max_lines crates/unifly/src/cli/commands/firewall/shared.rs 820

check_max_lines crates/unifly-api/src/controller/session_queries.rs 80
check_max_lines crates/unifly-api/src/controller/session_queries/admin.rs 120
check_max_lines crates/unifly-api/src/controller/session_queries/backups.rs 60
check_max_lines crates/unifly-api/src/controller/session_queries/common.rs 120
check_max_lines crates/unifly-api/src/controller/session_queries/raw.rs 100
check_max_lines crates/unifly-api/src/controller/session_queries/settings.rs 180
check_max_lines crates/unifly-api/src/controller/session_queries/stats.rs 120
check_max_lines crates/unifly-api/src/controller/session_queries/system.rs 200
check_max_lines crates/unifly-api/src/controller/session_queries/vpn.rs 900
check_max_lines crates/unifly-api/src/controller/session_queries/wifi.rs 90

check_max_lines crates/unifly-api/src/controller/refresh.rs 220
check_max_lines crates/unifly-api/src/controller/refresh/integration.rs 260
check_max_lines crates/unifly-api/src/controller/refresh/merge.rs 220
check_max_lines crates/unifly-api/src/controller/refresh/session.rs 220

check_no_complexity_allows crates/unifly/src/tui/widgets/hyperchart
check_no_complexity_allows crates/unifly/src/cli/commands/vpn.rs
check_no_complexity_allows crates/unifly/src/cli/commands/vpn
check_no_complexity_allows crates/unifly-api/src/controller/session_queries.rs
check_no_complexity_allows crates/unifly-api/src/controller/session_queries
check_no_complexity_allows crates/unifly-api/src/controller/refresh.rs
check_no_complexity_allows crates/unifly-api/src/controller/refresh

exit "$fail"
