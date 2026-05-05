# PoC #85 — Y-1 (claude CLI + stdio MCP server) verification result

## サマリー

- **s4 合否**: 10 / 10 pass
- **閾値 (>=8/10)**: ✅ established
- 実行日時: 2026-05-05T13:48:34Z
- claude version: 2.1.119 (Claude Code)
- model: claude-sonnet-4-6
- timeout/trial: 60s
- 平均 callLlm 呼び出し回数/trial: 1.60
- 平均 execCmd 呼び出し回数/trial: 2.60

## 検証項目

### 1. stdio MCP server が claude プロセス内で正しく spawn・終了するか

- **結果**: 全 10 試行で claude が `mcp__poc__*` tool を認識した（trial 集計表参照）
- **判定**: ✅ spawn・cleanup ともに claude 側で正しく処理されている
- **根拠**: 各 trial の stream-json 1 行目 `{type:"system",subtype:"init"}` に `mcp_servers:[{"name":"poc","status":"connected"}]` が含まれ、claude exit で子プロセスも自動終了することを目視確認

### 2. claude が MCP tool を意図通り使用するか

- **結果**: 全 10 試行で合計 callLlm 16 回 / execCmd 26 回 (avg 1.60 / 2.60)。各 trial で必ず submit_generated_code が最終 tool_use として現れた
- **判定**: ✅ 探索目的で callLlm / execCmd を呼び、submit_generated_code で最終出力する想定 sequence を再現
- **根拠**: trial 集計表の llm_count / exec_count 列、および各 trial の stream-json tool_use 順序

### 3. generate-skill-code 出力等価性 (構造合格率)

- **結果**: 10 / 10 (= 100%) が `{code, capabilities, schema}` の JSON 構造として整合
- **判定**: ✅ 80% 閾値クリア
- **根拠**: trial 集計表の submit_count / shape_ok / pass 列

### 4. エラー系挙動 (観察メトリックのみ)

- timeout (exit != 0): 0 / 10
- submit_generated_code 未呼び出し: 0 / 10
- submit はされたが構造不正: 0 / 10

## N=10 trial 集計

| trial# | elapsed (s) | exit code | submit_count | shape_ok | llm_count | exec_count | pass |
|---|---|---|---|---|---|---|---|
| 1 | 52 | 0 | 1 | 1 | 1 | 3 | 1 |
| 2 | 52 | 0 | 1 | 1 | 1 | 4 | 1 |
| 3 | 41 | 0 | 1 | 1 | 2 | 2 | 1 |
| 4 | 41 | 0 | 1 | 1 | 1 | 3 | 1 |
| 5 | 46 | 0 | 1 | 1 | 2 | 3 | 1 |
| 6 | 47 | 0 | 1 | 1 | 2 | 2 | 1 |
| 7 | 52 | 0 | 1 | 1 | 2 | 2 | 1 |
| 8 | 41 | 0 | 1 | 1 | 2 | 2 | 1 |
| 9 | 36 | 0 | 1 | 1 | 2 | 2 | 1 |
| 10 | 31 | 0 | 1 | 1 | 1 | 3 | 1 |

## 判定

**Y-1 成立** — 構造化出力が 80% 以上で取れ、claude が MCP tool を意図通り使い、stdio MCP server が正しく動く。#83 は Y-1 方針で詰められる。
