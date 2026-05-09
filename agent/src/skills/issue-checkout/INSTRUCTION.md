# Issue から Git ブランチを作成・切り替えする

GitHub Issue 番号（または URL）を受け取り、CONTRIBUTING.md のブランチ命名規則
`{prefix}/{issue-number}/{summary}` に沿った作業ブランチを生成・チェックアウトする。

## 入力

- `issueNumber: string` — GitHub Issue 番号または URL
- `base?: string` — 新ブランチを分岐させるベース ref（省略時は `main`）
  - ローカルブランチ名（例: `main`）でもリモートトラッキング ref（例: `origin/main`）でも可

## 処理フロー

### 1. Issue 情報を取得

`gh issue` コマンドを実行して Issue 本文を取得する。

- 例: `gh issue view <issueNumber>`
- 出力は `gh issue view` の生 stdout（タイトル / state / labels / body などを含むテキスト）
- このテキストから **タイトル** と **種別ラベル** を抽出する

### 2. ブランチ prefix を決定

種別ラベルから以下の対応表で prefix を決定する。

| ラベル          | prefix     |
| --------------- | ---------- |
| `enhancement`   | `feature`  |
| `bug`           | `fix`      |
| `chore`         | `chore`    |
| `documentation` | `docs`     |
| `refactor`      | `refactor` |
| `test`          | `test`     |
| `poc`           | `poc`      |

種別ラベルが付与されていない / 複数付与されている場合は処理を中止する
（output を呼ばずに loop を終了させる）。

### 3. summary を生成

Issue タイトルから英語 kebab-case の `summary` を生成する。

- 英小文字 + 数字 + ハイフンのみ
- 先頭・末尾・連続ハイフン禁止
- 簡潔かつ Issue の主旨が読み取れる粒度にする（3〜6 単語程度を目安）

### 4. ブランチ名を組み立てる

`{prefix}/{issueNumber}/{summary}` 形式で `branchName` を組み立てる。

`issueNumber` が URL 形式で渡された場合は、末尾の数字部分のみを使う。

### 5. ブランチ名を検証

`validate-branch-name` skill を呼び出して構造を検証する。

- 入力: `{ "branchName": "<branchName>" }`
- 出力: `{ valid: boolean, errors: string[] }`
- `valid: false` の場合は `errors` を踏まえて summary を生成し直し、再度検証する
- 数回の試行で valid にならない場合は処理を中止する

### 6. base ref を解決して存在を確認

入力 `base` があればそれを使用、なければ `main` をデフォルトとして使用する。

`git rev-parse --verify <base>` を実行して base ref の存在を確認する。

- 終了コード 0 なら存在（ローカルブランチでもリモートトラッキング ref でも可）
- 非 0 終了なら base が存在しないため処理を中止する

### 7. 既存ブランチとの衝突チェック

新ブランチ名がローカルとリモートの両方で存在しないことを確認する。

- ローカル: `git rev-parse --verify refs/heads/<branchName>` を実行
  - 終了コード 0（= 既存）なら衝突
- リモート: `git ls-remote --heads origin refs/heads/<branchName>` を実行
  - 出力が空でなければ衝突

衝突した場合は処理を中止する（自動 switch / suffix 付与は行わない）。

### 8. ブランチを作成・切り替え

`git checkout -b <branchName> <base>` を実行してブランチを作成し切り替える。

### 9. 結果を返す

output tool を呼び出して `{ "branchName": "<branchName>" }` を返す。

## 重要: 終了条件

- output tool は **ちょうど 1 回** だけ呼び出すこと。
- 処理を中止する場合（ラベル不整合 / 検証失敗 / 既存ブランチ衝突）は
  output を呼ばずに loop を終了させる（loop-exceeded で失敗扱いになる）。
- ツール呼び出しは合計 8〜10 回程度で完結する想定。試行錯誤しすぎない。
