# tokenbox — 配布設計

status: draft v0.1。配布経路の種類と成果物だけを書く。配布する側の基盤（社内の registry、CI の実行環境、対象ホスト）は、この repo の文書に書かない（README「文書の規約」）。

## 1. ビルド成果物

tag `v*` で CI を起動し、`cargo build --release` で各 target をビルドして Release に添付する。

| target | 成果物 |
|---|---|
| `aarch64-apple-darwin` | `tokenbox-v{ver}-aarch64-apple-darwin.tar.gz` |
| `x86_64-apple-darwin` | `tokenbox-v{ver}-x86_64-apple-darwin.tar.gz` |
| `x86_64-unknown-linux-gnu` | `tokenbox-v{ver}-x86_64-unknown-linux-gnu.tar.gz`、`tokenbox_{ver}_amd64.deb` |
| `aarch64-unknown-linux-gnu` | `tokenbox-v{ver}-aarch64-unknown-linux-gnu.tar.gz`、`tokenbox_{ver}_arm64.deb` |
| `x86_64-pc-windows-msvc` | `tokenbox-v{ver}-x86_64-pc-windows-msvc.zip`、`tokenbox-{ver}-x64.msi` |

- 各成果物に `sha256` を `SHA256SUMS` として添付。brew / winget / npm はこれを参照する。
- Linux は glibc 2.31 以上（Ubuntu 20.04 相当）を下限。musl 静的ビルドは v0.1 では採らない。当初の理由（`git` コマンドに依存する時点で動的環境前提）は、`git` コマンドを使わない決定（SPEC §6）で成り立たなくなったため、要否は未確定 D5。
- ビルドツール: `cargo-dist` を使う案と、手書き workflow の案。**手書き**にする（cargo-dist は apt/winget の面倒を見ないため、結局 4 経路のうち 2 つは自前になる）。
- CI: GitHub Actions（`unagents/tokenbox`）が mac の成果物・GitHub Release・brew・npm を担当する。Linux の成果物と `.deb` をどの CI で作るかは、配布する側の基盤に依存する。
- workflow 定義には、配布する側の基盤の URL・ホスト名・認証を直書きしない。CI の変数・secret で渡す。

## 2. macOS — Homebrew tap

- repo: GitHub `unagents/homebrew-tap`。
- tokenbox は CLI なので **Formula**（`Formula/tokenbox.rb`）。Casks ではない。
- Formula は Release の tar.gz を `url` + `sha256` で指す（ソースからビルドしない。利用側に Rust を要求しない）。arm64 / x86_64 を `on_arm` / `on_intel` で分ける。
- 更新: Release workflow の最後に、tap repo へ Formula を書き換える commit を push（`GITHUB_TOKEN` に tap への write が要る → tap 用の fine-grained PAT を secrets に置く）。
- 利用: `brew tap unagents/tap && brew install tokenbox`。

## 3. Linux — apt と tar.gz

- `.deb` は `cargo-deb` で作る。
- apt repository の置き場、publish の方法、利用側の `sources.list` の設定は、配布する側の基盤に依存する。この repo の文書には書かない。
- `.rpm` は作らない。apt を使わない環境は tar.gz を直置きする:
  ```
  V=0.1.0; T=$(uname -m | sed 's/x86_64/x86_64/; s/aarch64/aarch64/')-unknown-linux-gnu
  curl -fsSL -o /tmp/tb.tgz https://github.com/unagents/tokenbox/releases/download/v$V/tokenbox-v$V-$T.tar.gz
  curl -fsSL https://github.com/unagents/tokenbox/releases/download/v$V/SHA256SUMS | grep "tokenbox-v$V-$T.tar.gz" | sed 's#  .*#  /tmp/tb.tgz#' | sha256sum -c -
  sudo tar -xzf /tmp/tb.tgz -C /usr/local/bin tokenbox && rm /tmp/tb.tgz
  ```

## 4. Windows — winget

- winget は Microsoft の community repo（`microsoft/winget-pkgs`）に PR を出す方式が標準。v0.1 では掲載しない。
- 代わりに **private winget source**（REST source）を立てるか、`winget install --manifest <path>` でローカル manifest から入れる。
  - v0.1: **ローカル manifest**（`winget/tokenbox.yaml` を Release に添付、`winget install --manifest` で入れる）。
  - private source は需要が出たら検討（未確定 D2）。
- MSI は `cargo-wix` で生成。インストール先 `%ProgramFiles%\tokenbox\`、PATH 追加。

## 5. npx — `@unagents/tokenbox`

本体パッケージ + プラットフォーム別パッケージの optionalDependencies 構成にする。

```
npm/
  tokenbox/                 @unagents/tokenbox      (bin: tokenbox → bin/run.js)
  tokenbox-darwin-arm64/    @unagents/tokenbox-darwin-arm64   (バイナリ同梱)
  tokenbox-darwin-x64/
  tokenbox-linux-x64/
  tokenbox-linux-arm64/
  tokenbox-win32-x64/
```

- `bin/run.js`: `process.platform` / `process.arch` から該当パッケージを `require.resolve` し、`spawnSync` で stdio を継承して実行。見つからなければ「このプラットフォームは未配布」と終了コード 1。
- postinstall で外部からダウンロードしない（企業プロキシ・オフライン・改ざんの問題を避ける）。optionalDependencies 方式に確定（SPEC U5）。
- publish 先: npmjs（public）。それ以外の registry に出すかどうかは、配布する側の基盤に依存する。
- 無人起動は `npx` を避け、`brew` か tar.gz 直置きのバイナリを呼ぶ（npx は初回に network と node が要る）。npx は人が手で使う経路。
- 利用: `npx @unagents/tokenbox <git-url> get <key-name>`。

## 6. バージョンと互換

- SemVer。`Cargo.toml` の version が正本。tag `v{ver}` を切ると全経路が同じ版で出る。
- store の形式（SPEC §4）に版を持たせ、バイナリは自分が扱える版かを `register`/`get` の最初に確認する。版の置き方は未確定（SPEC §10 U7）。

## 7. v0.1 の配布範囲（決定 2026-11-21）

| 経路 | v0.1 |
|---|---|
| brew tap（macOS） | **対象** |
| apt（`.deb`） | **対象** |
| npx | **対象** |
| tar.gz 直置き | **対象** |
| winget（Windows） | 保留 |

## 8. 未確定

- D2: winget は保留のため未決。
- D3: brew tap の更新に使う PAT の置き場（GitHub secrets でよいか）。
- D5: musl 静的ビルドの要否と、`git2`（libgit2 同梱）を各 target でクロスビルドできるか（着手時に実測）。

D1・D4 は配布する側の基盤の事項のため、この文書から外した（2026-11-21）。
