# Contributing

このドキュメントは本リポジトリにおける運用ルールの正本である。
人間と AI エージェントの双方が同じルールに従って作業する。

---

## ブランチ運用

### ブランチモデル

本リポジトリは **トランクベース** を採用する。

- `main` を唯一の長期ブランチとする。
- 作業ブランチは `main` から分岐し、Pull Request を経て `main` にマージする。
- `main` への直接 commit は禁止する。すべての変更は作業ブランチ + Pull Request を経由して取り込む。

### ブランチ命名規則

ブランチ名は以下の形式で統一する。

```
{prefix}/{issue-number}/{summary}
```

例: `chore/22/define-branching-rules`

#### `{prefix}`

| prefix     | 用途                                                           |
| ---------- | -------------------------------------------------------------- |
| `feature`  | 新機能の追加                                                   |
| `fix`      | バグ修正                                                       |
| `chore`    | 雑務・セットアップ・非機能改善                                 |
| `docs`     | ドキュメントの追加・修正                                       |
| `refactor` | 振る舞いを変えないコード整理                                   |
| `test`     | テストの追加・修正                                             |
| `poc`      | 設計の前提を実機で検証する PoC（Issue ラベル `poc` と対応する） |

#### `{issue-number}`

- **必須**。すべての作業は Issue を起点に行う（例外なし）。
- 対応する GitHub Issue 番号をそのまま記載する。

#### `{summary}`

- **英語 + kebab-case** で簡潔に記述する。
- ブランチで取り組む内容が一目でわかる粒度にする。

---

## コミットメッセージ規約

[Conventional Commits](https://www.conventionalcommits.org/) に準拠する。

### prefix（type）

利用可能な type は以下のとおり。

- `feat` / `fix` / `chore` / `docs` / `refactor` / `test` / `perf` / `build` / `ci`

ブランチ規則の `feature` プレフィックスはコミットでは `feat` に対応する（[対応表](#ラベル--ブランチ-prefix--コミット-type-の対応)）。

### subject（1 行目）

形式: `<type>: <imperative subject>`

- 言語: **英語**
- 文体: **命令形**（`add` / `fix` / `move` ...）
- 文字数: **50 文字目安**（type を含む）

### body（任意）

- subject だけで意図が伝わる場合は省略してよい。
- 背景・動機・代替案など、subject では伝わらない情報があるときに記載する。
- subject との間に空行を 1 行入れる。

### footer

PR 経由で Issue を解決する場合、footer に Issue 参照を記載する。

- PR マージ時に Issue を自動 close したい場合: `Closes #N`
- 関連はあるが PR で完結しない（親 Issue / 設計チケット 等）場合: `Refs #N`

### breaking change

互換性を破る変更（breaking change）の表記ルールは現時点では規定しない。
公開 API が確定したタイミングで別 Issue として整備する。

---

## Pull Request 運用

### タイトル

コミット subject と同一規約とする。

```
<type>: <imperative subject>
```

Squash マージにより PR タイトルがそのまま `main` 上のコミット subject になるため、
コミット規約と PR タイトル規約は一致させる。

### 本文テンプレート

軽量構造に統一する。

```markdown
## 概要

<変更の背景・目的・主な変更点>

Closes #N
```

- `## 概要` セクションを必ず置く。
- 末尾に関連 Issue を記載する（`Closes #N` または `Refs #N`）。

### マージ方式

**Squash マージで統一する**（例外なし）。

- 1 PR = 1 commit を `main` に積む。
- PR タイトルがそのまま squash 後のコミット subject になる。
- マージ時に作業ブランチは削除する（`gh pr merge --squash --delete-branch`）。

### レビュー

- セルフマージを許容する（人間レビュアーは必須としない）。
- 必要に応じて他者にレビュー依頼を行う運用を妨げない。

### Draft PR

作業中で議論はしたいがマージ可能ではない PR は Draft で作成する。
レビュー可能な状態になったら Ready for review に切り替える。

---

## Issue 運用

### Issue テンプレート

`.github/ISSUE_TEMPLATE/` は置かない。
本ドキュメントの規約に従って、対話 skill または手動で起票する。

### ラベル方針

ラベルは「**種別**」と「**状態**」の 2 軸を規約化する。
それ以外（`good first issue` / `help wanted` / `question` / `wontfix` / `duplicate` / `invalid` 等）は任意で使用する。

#### 種別ラベル（必須・1 つ付与）

起票時に必ず 1 つ付与する。

| ラベル          | 用途                                                           |
| --------------- | -------------------------------------------------------------- |
| `enhancement`   | 新機能の追加                                                   |
| `bug`           | バグ修正                                                       |
| `chore`         | 雑務・セットアップ・非機能改善                                 |
| `documentation` | ドキュメントの追加・修正                                       |
| `refactor`      | 振る舞いを変えないコード整理                                   |
| `test`          | テストの追加・修正                                             |
| `poc`           | 設計の前提を実機で検証する PoC                                 |

#### 状態ラベル

| ラベル         | 付与基準                                                       |
| -------------- | -------------------------------------------------------------- |
| `design-fixed` | `implementation-check` skill で「実装可能」と判定されたとき    |

### 状態遷移

```
起票（種別ラベル付与）
  ↓
設計合意（design-fixed 付与）
  ↓
作業ブランチ作成・PR 作成
  ↓
PR マージ（Closes #N により自動 close）
```

### ラベル ↔ ブランチ prefix ↔ コミット type の対応

| ラベル          | ブランチ prefix | コミット type                |
| --------------- | --------------- | ---------------------------- |
| `enhancement`   | `feature`       | `feat`                       |
| `bug`           | `fix`           | `fix`                        |
| `chore`         | `chore`         | `chore`                      |
| `documentation` | `docs`          | `docs`                       |
| `refactor`      | `refactor`      | `refactor`                   |
| `test`          | `test`          | `test`                       |
| `poc`           | `poc`           | 作業内容に応じた type を選ぶ |
