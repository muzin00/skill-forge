#!/usr/bin/env bash
set -euo pipefail

# PoC #87: real-handler MCP で生成 skill の意味的合格 (s5) 検証
# 各 trial:
#   1. mktemp 隔離 git repo を作成し cwd 切替
#   2. claude exploration (real callLlm / execCmd) → submit_generated_code
#   3. submit input から code/schema を tmpdir/skill/ に書き出し
#   4. skill-forge run --skill ... --issue-number 1 を subprocess 起動
#   5. exit 0 + stdout の最終行が JSON.parse 可能なら s5_pass

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MCP_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_ROOT="$(cd "$MCP_DIR/.." && pwd)"
MCP_BIN="$MCP_DIR/target/release/mcp-poc-server"
SKILL_FORGE_BIN="$REPO_ROOT/target/release/skill-forge"
PROMPT_FILE="$SCRIPT_DIR/prompt.txt"
N=${N:-5}
TIMEOUT_SECS=${TIMEOUT_SECS:-60}
MODEL=${MODEL:-claude-sonnet-4-6}
ISSUE_NUMBER=${ISSUE_NUMBER:-1}

echo "[run-s5] building mcp-poc-server..." >&2
cargo build --release --manifest-path "$MCP_DIR/Cargo.toml" >&2
echo "[run-s5] building skill-forge..." >&2
cargo build --release --manifest-path "$REPO_ROOT/Cargo.toml" >&2

if [ -f "$REPO_ROOT/.env" ]; then
  set -a
  # shellcheck disable=SC1091
  source "$REPO_ROOT/.env"
  set +a
fi
if [ -z "${ANTHROPIC_API_KEY:-}" ]; then
  echo "[run-s5] ANTHROPIC_API_KEY is required" >&2
  exit 2
fi

export GH_REPO=muzin00/skill-forge
export MCP_POC_MODE=real
export MCP_LLM_MODEL="$MODEL"

WORK_DIR=$(mktemp -d -t skill-forge-poc-87.XXXXXX)
trap 'rm -rf "$WORK_DIR"' EXIT
CONFIG_FILE="$WORK_DIR/mcp-config.json"
cat > "$CONFIG_FILE" <<JSON
{
  "mcpServers": {
    "poc": {
      "command": "$MCP_BIN",
      "args": []
    }
  }
}
JSON

PROMPT_BODY=$(cat "$PROMPT_FILE")

# camelCase → kebab-case (matches src/validator.rs:camel_to_kebab)
camel_to_kebab() {
  printf '%s' "$1" | sed -E 's/([a-z0-9])([A-Z])/\1-\2/g' | tr '[:upper:]' '[:lower:]'
}

run_trial() {
  local i=$1
  local trial_dir="$WORK_DIR/trial-$i"
  mkdir -p "$trial_dir/skill"
  (cd "$trial_dir" && git init -q && git -c user.email=poc@test -c user.name=poc commit --allow-empty -m init -q)

  local out="$trial_dir/claude.jsonl"
  local err="$trial_dir/claude.err"

  local start
  start=$(date +%s)
  set +e
  (
    cd "$trial_dir"
    perl -e 'alarm shift; exec @ARGV or die "exec: $!"' "$TIMEOUT_SECS" \
      claude -p --bare --strict-mcp-config --mcp-config "$CONFIG_FILE" \
        --disallowedTools "Bash Edit Read" \
        --output-format stream-json --verbose --model "$MODEL" \
        "$PROMPT_BODY"
  ) > "$out" 2> "$err"
  local claude_exit=$?
  set -e
  local claude_elapsed=$(( $(date +%s) - start ))

  # s4 判定: submit_generated_code の input 抽出 + 構造チェック
  local submit_inputs=""
  local submit_count=0
  local s4_pass=0
  if [ -s "$out" ]; then
    submit_inputs=$(jq -c 'select(.type=="assistant") | .message.content[]? | select(.type=="tool_use" and .name=="mcp__poc__submit_generated_code") | .input' "$out" 2>/dev/null || true)
    if [ -n "$submit_inputs" ]; then
      submit_count=$(printf '%s\n' "$submit_inputs" | grep -c . || true)
    fi
    if [ "$submit_count" = "1" ]; then
      if printf '%s' "$submit_inputs" | jq -e 'has("code") and has("capabilities") and has("schema") and (.code|type=="string") and (.capabilities|type=="array") and (.schema|type=="object")' > /dev/null 2>&1; then
        s4_pass=1
      fi
    fi
  fi

  local llm_count
  local exec_count
  llm_count=$(jq -c 'select(.type=="assistant") | .message.content[]? | select(.type=="tool_use" and .name=="mcp__poc__callLlm")' "$out" 2>/dev/null | grep -c . || true)
  exec_count=$(jq -c 'select(.type=="assistant") | .message.content[]? | select(.type=="tool_use" and .name=="mcp__poc__execCmd")' "$out" 2>/dev/null | grep -c . || true)

  # s5 判定: 生成 skill を skill-forge run で実行
  local s5_pass=0
  local run_exit=-1
  local run_elapsed=0
  local skill_total_elapsed=$claude_elapsed
  if [ "$s4_pass" = "1" ]; then
    local code schema
    code=$(printf '%s' "$submit_inputs" | jq -r '.code')
    schema=$(printf '%s' "$submit_inputs" | jq -c '.schema')
    local capabilities
    capabilities=$(printf '%s' "$submit_inputs" | jq -r '.capabilities | join(", ")')

    {
      printf '// capabilities: %s\n' "$capabilities"
      printf '%s' "$code"
    } > "$trial_dir/skill/skill.js"
    {
      printf 'defineSchema('
      printf '%s' "$schema" | jq -S .
      printf ');\n'
    } > "$trial_dir/skill/schema.js"

    # input flag derivation: schema.properties の最初の key を kebab 化
    local flag_key
    flag_key=$(printf '%s' "$schema" | jq -r '.properties | keys[0] // empty')
    local flag_kebab
    flag_kebab=$(camel_to_kebab "$flag_key")

    local remain=$(( TIMEOUT_SECS - claude_elapsed ))
    if [ "$remain" -lt 5 ]; then
      remain=5
    fi

    local run_start
    run_start=$(date +%s)
    set +e
    (
      cd "$trial_dir"
      perl -e 'alarm shift; exec @ARGV or die "exec: $!"' "$remain" \
        "$SKILL_FORGE_BIN" run \
          --skill "$trial_dir/skill/skill.js" \
          --model "$MODEL" \
          "--$flag_kebab" "$ISSUE_NUMBER"
    ) > "$trial_dir/run.out" 2> "$trial_dir/run.err"
    run_exit=$?
    set -e
    run_elapsed=$(( $(date +%s) - run_start ))
    skill_total_elapsed=$(( claude_elapsed + run_elapsed ))

    if [ "$run_exit" = "0" ] && [ -s "$trial_dir/run.out" ]; then
      if tail -n 1 "$trial_dir/run.out" | jq -e . > /dev/null 2>&1; then
        s5_pass=1
      fi
    fi
  fi

  # idx,total_elapsed,claude_exit,run_exit,s4_pass,s5_pass,llm_count,exec_count,flag_key
  echo "$i,$skill_total_elapsed,$claude_exit,$run_exit,$s4_pass,$s5_pass,$llm_count,$exec_count,${flag_key:-N/A}"
}

TRIAL_RESULTS=()
for i in $(seq 1 "$N"); do
  echo "[run-s5] trial $i/$N..." >&2
  row=$(run_trial "$i")
  TRIAL_RESULTS+=("$row")
  echo "[run-s5] trial $i: $row" >&2
done

# 集計
s4_total=0
s5_total=0
llm_total=0
exec_total=0
timeout_count=0
no_submit_count=0
shape_fail_count=0
run_fail_count=0
for row in "${TRIAL_RESULTS[@]}"; do
  IFS=, read -r idx elapsed claude_exit run_exit s4 s5 llm exec flag_key <<< "$row"
  s4_total=$((s4_total + s4))
  s5_total=$((s5_total + s5))
  llm_total=$((llm_total + llm))
  exec_total=$((exec_total + exec))
  if [ "$claude_exit" != "0" ]; then
    timeout_count=$((timeout_count + 1))
  fi
  if [ "$s4" = "0" ]; then
    no_submit_count=$((no_submit_count + 1))
  fi
  if [ "$s4" = "1" ] && [ "$s5" = "0" ]; then
    if [ "$run_exit" = "0" ]; then
      shape_fail_count=$((shape_fail_count + 1))
    else
      run_fail_count=$((run_fail_count + 1))
    fi
  fi
done

threshold=$(( N * 4 / 5 ))
verdict="not established"
verdict_emoji="❌"
if [ "$s5_total" -ge "$threshold" ]; then
  verdict="established"
  verdict_emoji="✅"
fi

RESULT_FILE="$MCP_DIR/result-s5.md"
{
  echo "# PoC #87 — real-handler MCP + skill-runtime 実行 (s5) verification result"
  echo
  echo "## サマリー"
  echo
  echo "- **s4 (構造合格)**: $s4_total / $N"
  echo "- **s5 (実行成功)**: $s5_total / $N"
  echo "- **閾値 (>=$threshold/$N = 80%)**: $verdict_emoji $verdict"
  echo "- 実行日時: $(date -u +"%Y-%m-%dT%H:%M:%SZ")"
  echo "- claude version: $(claude --version 2>/dev/null || echo unknown)"
  echo "- model: $MODEL"
  echo "- timeout/trial: ${TIMEOUT_SECS}s (claude exploration + skill-forge run 合算)"
  echo "- 代表 input: GitHub Issue #$ISSUE_NUMBER (GH_REPO=$GH_REPO)"
  echo
  echo "## 検証項目"
  echo
  echo "### 1. real-handler 経由で claude が探索ループを正しく収束させるか"
  echo
  echo "- 全 $N 試行で合計 callLlm $llm_total 回 / execCmd $exec_total 回 (avg $(awk -v t="$llm_total" -v n="$N" 'BEGIN{printf"%.2f",t/n}') / $(awk -v t="$exec_total" -v n="$N" 'BEGIN{printf"%.2f",t/n}'))"
  echo "- s4 合格: $s4_total / $N"
  echo "- **判定**: $([ "$s4_total" = "$N" ] && echo "✅ 全 trial で構造合格" || echo "⚠️ 構造合格率に揺れ ($s4_total/$N)")"
  echo
  echo "### 2. 生成 skill が skill-runtime 上で実行可能か"
  echo
  echo "- skill-forge run 成功 (exit 0 + JSON stdout): $s5_total / $N"
  echo "- **判定**: $([ "$s5_total" -ge "$threshold" ] && echo "✅ 80% 閾値クリア" || echo "❌ 80% 閾値未達")"
  echo
  echo "### 3. capabilities 宣言と実際の使用が一致するか (定性)"
  echo
  echo "- 生成 skill は \`gh issue view\` (= execCmd) と要約・命名 (= callLlm) を使う性質上、capabilities = \`[\"callLlm\", \"execCmd\"]\` の宣言が期待される"
  echo "- s5 (skill-forge run) が exit 0 + JSON stdout で $s5_total/$N 成功している事実から、capabilities 宣言は実際の host primitive 使用と矛盾しなかったと判断（厳密な静的照合は本 PoC のスコープ外）"
  echo
  echo "### 4. エラー系挙動 (観察メトリック)"
  echo
  echo "- claude プロセス timeout/異常終了: $timeout_count / $N"
  echo "- submit_generated_code 未呼び出し or 構造不正: $no_submit_count / $N"
  echo "- skill-forge run プロセスが exit != 0: $run_fail_count / $N"
  echo "- skill-forge run は 0 だが stdout が JSON でない: $shape_fail_count / $N"
  echo
  echo "## N=$N trial 集計"
  echo
  echo "| trial# | elapsed (s) | claude exit | run exit | s4_pass | s5_pass | llm | exec | flag_key |"
  echo "|---|---|---|---|---|---|---|---|---|"
  for row in "${TRIAL_RESULTS[@]}"; do
    IFS=, read -r idx elapsed claude_exit run_exit s4 s5 llm exec flag_key <<< "$row"
    echo "| $idx | $elapsed | $claude_exit | $run_exit | $s4 | $s5 | $llm | $exec | $flag_key |"
  done
  echo
  echo "## 判定"
  echo
  if [ "$s5_total" -ge "$threshold" ]; then
    echo "**s5 成立** — real-handler 経由で生成された skill が skill-runtime 上で意味的にも実行可能であることを確認。#83 を Y-1 方針で本実装フェーズへ進められる。"
  else
    echo "**s5 不成立** — 80% 閾値未達。prompt / handler 側の調整で再試行、または #83 で別案検討。"
  fi
} > "$RESULT_FILE"

echo "[run-s5] wrote $RESULT_FILE" >&2
echo "[run-s5] verdict: $verdict_emoji $verdict (s4 $s4_total/$N, s5 $s5_total/$N)" >&2
