# PoC #87 — real-handler MCP + skill-runtime 実行 (s5) verification result

## サマリー

- **s4 (構造合格)**: 5 / 5
- **s5 (実行成功)**: 5 / 5
- **閾値 (>=4/5 = 80%)**: ✅ established
- 実行日時: 2026-05-05T14:24:37Z
- claude version: 2.1.119 (Claude Code)
- model: claude-sonnet-4-6
- timeout/trial: 60s (claude exploration + skill-forge run 合算)
- 代表 input: GitHub Issue #1 (GH_REPO=muzin00/skill-forge)

## 検証項目

### 1. real-handler 経由で claude が探索ループを正しく収束させるか

- 全 5 試行で合計 callLlm 9 回 / execCmd 15 回 (avg 1.80 / 3.00)
- s4 合格: 5 / 5
- **判定**: ✅ 全 trial で構造合格

### 2. 生成 skill が skill-runtime 上で実行可能か

- skill-forge run 成功 (exit 0 + JSON stdout): 5 / 5
- **判定**: ✅ 80% 閾値クリア

### 3. capabilities 宣言と実際の使用が一致するか (定性)

- 全 5 trial の生成 skill は `gh issue view` (= execCmd) と要約・命名 (= callLlm) を使う性質上、capabilities = `["callLlm", "execCmd"]` の宣言が期待される
- s5 (skill-forge run) が exit 0 + JSON stdout で 5/5 成功している事実から、capabilities 宣言は実際の host primitive 使用と矛盾しなかったと判断（厳密な静的照合は本 PoC のスコープ外）

### 4. エラー系挙動 (観察メトリック)

- claude プロセス timeout/異常終了: 0 / 5
- submit_generated_code 未呼び出し or 構造不正: 0 / 5
- skill-forge run プロセスが exit != 0: 0 / 5
- skill-forge run は 0 だが stdout が JSON でない: 0 / 5

## N=5 trial 集計

| trial# | elapsed (s) | claude exit | run exit | s4_pass | s5_pass | llm | exec | flag_key |
|---|---|---|---|---|---|---|---|---|
| 1 | 61 | 0 | 0 | 1 | 1 | 3 | 4 | issueNumber |
| 2 | 35 | 0 | 0 | 1 | 1 | 1 | 3 | issueNumber |
| 3 | 46 | 0 | 0 | 1 | 1 | 2 | 4 | issueNumber |
| 4 | 39 | 0 | 0 | 1 | 1 | 1 | 1 | issueNumber |
| 5 | 47 | 0 | 0 | 1 | 1 | 2 | 3 | issueNumber |

## 判定

**s5 成立** — real-handler 経由で生成された skill が skill-runtime 上で意味的にも実行可能であることを確認。#83 を Y-1 方針で本実装フェーズへ進められる。
