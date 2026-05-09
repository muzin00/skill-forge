# 現在のブランチから GitHub Pull Request を作成する

現在のブランチ（`{prefix}/{issue-number}/{summary}` 形式）の変更を `gh pr create` で
Pull Request にする。CONTRIBUTING.md の PR 規約に従ってタイトル・本文を組み立てる。

## 入力

- `base?: string` — PR のベースブランチ（ローカルブランチ名でもリモートトラッキング ref でも可）。省略時は `main`。

## 処理フロー

### 1. 現在のブランチを取得

`git rev-parse --abbrev-ref HEAD` を実行して現在のブランチ名を取得する。

- 結果が `main` の場合は処理を中止する（`main` から PR は作成しない）。
- 結果が `HEAD`（detached）の場合も処理を中止する。

### 2. ブランチ名から Issue 番号と prefix を抽出

ブランチ名は `{prefix}/{issue-number}/{summary}` 形式（CONTRIBUTING.md）。
スラッシュ区切りで分解し、`prefix`（先頭セグメント）と `issue-number`（2 番目のセグメント）を抽出する。

- 抽出できない / Issue 番号が数値でない場合は処理を中止する（規約外ブランチからは PR を作成しない）。
- `prefix` は PR タイトルの type 決定に使う（次の対応表）。

| ブランチ prefix | コミット type                |
| --------------- | ---------------------------- |
| `feature`       | `feat`                       |
| `fix`           | `fix`                        |
| `chore`         | `chore`                      |
| `docs`          | `docs`                       |
| `refactor`      | `refactor`                   |
| `test`          | `test`                       |
| `poc`           | 作業内容に応じた type を選ぶ |

### 3. base ref を解決して存在を確認

入力 `base` があればそれを使用、なければ `main` をデフォルトとして使用する。

`git rev-parse --verify <base>` を実行して base ref の存在を確認する。

- 終了コード 0 なら存在（ローカルブランチでもリモートトラッキング ref でも可）。
- 非 0 終了なら base が存在しないため処理を中止する（フォールバック試行はしない）。

### 4. 変更内容を把握

ベースからの差分を取得する。

- `git log <base>..HEAD --oneline` でコミットの一覧を取得
- `git diff <base>...HEAD` で実際の変更内容を取得（出力が大きい場合は要約に十分な範囲だけ確認すればよい）

### 5. PR タイトルを生成

形式: `<type>: <imperative subject>`

- 言語: 英語
- 文体: 命令形（`add` / `fix` / `move` ...）
- 50 文字目安、長くても 70 文字以内
- 変更内容を簡潔に表す
- `<type>` は手順 2 の対応表で決める

### 6. PR 本文を生成

形式（Markdown）:

```
## 概要

<変更の背景・目的・主な変更点を簡潔に>

Closes #<issue-number>
```

- 末尾の Issue 参照は手順 2 で抽出した Issue 番号を使う。
- PR で Issue が完全には close されない場合（親 Issue / 設計チケット 等）は `Refs #<issue-number>` に置き換える。

### 7. リモートに push

`git push -u origin HEAD` を実行する。

- 既に最新まで push 済みなら "Everything up-to-date" が返るため副作用はない。
- upstream が未設定なら設定して push する。

### 8. PR を作成

`gh pr create --base <base> --title <title> --body <body>` を実行する。

- ベースには入力 `base`（省略時は `main`）を渡す。
- stdout に PR URL が出力されるので、それを `prUrl` として保持する。
- 出力に複数行が含まれる場合は最後の `https://...` 行を採用する。

### 9. 結果を返す

output tool を呼び出して `{ "prUrl": "<URL>" }` を返す。

## 重要: 終了条件

- output tool は **ちょうど 1 回** だけ呼び出すこと。
- 処理を中止する場合（`main` 上 / detached / 規約外ブランチ / base ref 不在 / push 失敗 等）は
  output を呼ばずに loop を終了させる（loop-exceeded で失敗扱いになる）。
- ツール呼び出しは合計 6〜10 回程度で完結する想定。試行錯誤しすぎない。
