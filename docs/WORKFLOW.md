# tokenbox — 作業手順

status: v0.1.0（2026-11-21）。§2〜§3 は実施済み、§4 は mac と Linux の tar.gz・brew まで実施済み（`.deb`・npm・winget は未着手）。配布する側の基盤（社内の registry、CI の実行環境、対象ホスト）での作業は、この repo の文書に書かない（README「文書の規約」）。

## 0. 前提の確認（着手前）

- [x] GitHub `unagents` org と `homebrew-tap` が存在する（確認済み）。
- [x] Rust toolchain（stable）。クロスビルドは CI に任せ、手元は host target のみ。
- [x] 決定済み: U5（npm）= optionalDependencies、D2 = 保留、D3 = GitHub secrets。U3 = Windows 保留。SPEC v0.2（2026-11-21）で未確定の番号を振り直した。U1（暗号の方式）・U2（push の競合）・U8（credential helper）は実装前に決める（SPEC §10）。
- [ ] 着手時に読むもの: `docs/SPEC.md` → `docs/DISTRIBUTION.md` → このファイル。`docs/DECISIONS.md` は理由の参照用。

## 1. repo を用意する（**実施済み 2026-11-20**）

1. [x] GitHub `unagents/tokenbox`（public。CI）。

確認: 済。

## 2. Rust プロジェクトの骨組み

```
tokenbox/
  Cargo.toml            # edition 2021, bin: tokenbox
  src/
    main.rs             # clap でサブコマンド
    cli.rs
    error.rs            # 失敗の種類と終了コード
    store.rs            # git2 で取得・push。受け皿は一時 bare repo（SPEC §6）
    crypto.rs           # key 名からファイル名と鍵を導出、暗号化・復号（SPEC §4）
    input.rs            # register の入力（隠し入力 / --clipboard / --stdin）
  tests/e2e.rs          # ローカルの bare repo を store にした E2E
  npm/                  # §DISTRIBUTION 5
  packaging/
    deb/                # control, postinst
    winget/
    brew/Formula.tokenbox.rb.tmpl
  .github/workflows/release.yml
  docs/
```

主要 crate: `clap`（derive）、`git2`、`secrecy`（値を `Debug` で伏せる）、`rpassword`（隠し入力）、`argon2`、`chacha20poly1305`、`tempfile`。

確認: `cargo build` が通り、`tokenbox --help` がサブコマンド一覧を出す。

## 3. 実装の順（毎回 `cargo test` と手元の E2E を回す）

1. `crypto`（key 名からファイル名と鍵を導出、暗号化・復号。SPEC §10 U1 を決めてから）。
2. `store`（`git2` で一時 bare repo に depth 1 で取得。無認証。認証を要求されたら終了コード 4）。
3. `get`。
4. `register`（隠し入力 / `--stdin`、commit、push。push の認証はホストの git 設定から読む。SPEC §10 U8）。
5. `register --clipboard`（SPEC §10 U6）。
6. Windows（CI の windows runner でテスト。実機は後回し）。

E2E（手元、一時ディレクトリで）: bare repo を store に見立て、`register` → 別の作業ディレクトリから `get` の値が一致。違う key 名の `get` は終了コード 2。実行後、ホストに受け皿が残っていない。

確認: E2E が通る（`cargo test`）。

## 4. 配布を組む（v0.1 の範囲は DISTRIBUTION §7）

1. GitHub `.github/workflows/release.yml`: tag で mac 2 target をビルド → GitHub Release 作成 → SHA256SUMS → brew Formula を tap へ push（D3: PAT は GitHub secrets）→ npm publish。Windows ジョブは書くが `if: false`（保留）。
2. Linux 2 target をビルド → `cargo-deb` で `.deb` → Release に tar.gz を添付。実行する CI と apt repository への publish は、配布する側の基盤で行う。
3. tar.gz 直置き手順（DISTRIBUTION §3）を README に載せる。
4. 後回し: winget（`cargo-wix`）。設計は DISTRIBUTION に残す。

確認: `brew install unagents/tap/tokenbox` → `tokenbox --version`。apt 経路で入れて `tokenbox <git-url> get <key-name>` が通る。tar.gz 直置きが通る。`npx @unagents/tokenbox --version` が動く。

## 5. 運用への切り替え・日常の運用・秘密の投入

利用側のネットワーク内の管理であり、この repo の文書には書かない（SPEC §9、README「文書の規約」）。2026-11-21 にここから外した。
