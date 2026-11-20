# tokenbox — 決定の記録

| 日付 | 決定 | 理由 | 代替案 |
|---|---|---|---|
| 2026-11-20 | 実装は Rust 単一バイナリ。mac / Linux / Windows。brew tap・apt・winget・npx の 4 経路 | ユーザー指示 | bash スクリプトのまま |
| 2026-11-20 | git 操作は外部 `git` コマンド。libgit2 を抱えない（**2026-11-21 変更**: `git2` を採用。下の行を参照） | 既存の credential helper 設定をそのまま使える。バイナリが小さい | `git2` crate |
| 2026-11-20 | age は `age` crate を直接リンク。外部 `age` コマンドに依存しない（**2026-11-21 変更**: recipients 方式を廃止。暗号の方式は未確定、SPEC §10 U1） | ホストに age を入れる手間を無くす。Windows でも同じ | 外部コマンド呼び出し |
| 2026-11-20 | npm は optionalDependencies 方式 | postinstall ダウンロードはプロキシ・オフライン・改ざんの問題がある | postinstall で Release から取得 |
| 2026-11-20 | winget は v0.1 ではローカル manifest | 検証できる実機が無い。private source は過剰 | microsoft/winget-pkgs へ PR、private REST source |
| 2026-11-20 | tokenbox は store の場所を持たない。`--store <git-url>` 必須、既定値なし（**2026-11-21 変更**: `--store` を廃止し位置引数に。下の行を参照） | tokenbox は仕組みのみ。置き場を固定するとプラットフォームとして使えない（ユーザー指摘） | 既定 URL を埋め込む |
| 2026-11-20 | store の private/public は store 側の運用判断。tokenbox は git 認証を持たず、利用側の git 設定に任せる（**2026-11-21 変更**: store は public 必須。下の行を参照） | 鶏と卵（初回 pull の認証）は tokenbox が解く問題ではない | 初回 init でトークン入力 / SSH ブートストラップ |
| 2026-11-20 | Windows は保留。実機が用意できた時点で検証 | 検証できる実機が無い | — |
| 2026-11-21 | 起動形は `tokenbox <git-url> <command>`。git URL は毎回必須の位置引数。`--store` は廃止 | 必須で既定値の無い値にフラグ名は要らない。全コマンドで URL の位置が揃う | `--store <git-url>`、`tokenbox <command> <git-url>` |
| 2026-11-21 | 操作は `register` と `get` の 2 つのみ。ホスト鍵・recipients・`meta/*.toml`・配置（`apply`）・ホストの登録と削除（`init` / `enroll` / `add` / `remove`）・`list` / `status` / `rotate-check` は持たない | tokenbox は仕組みのみ。誰が起動し、何に使い、どこに置くかは利用側（ユーザー指示） | ホスト鍵と recipients を使う方式と `apply` |
| 2026-11-21 | 読める条件は git URL と key 名の両方を知っていること。store のファイル名は key 名から導出し、値は key 名から導出した鍵で暗号化する。元の key 名は store に置かない。git URL は鍵の材料にしない | ユーザー指示。URL の表記が変わっても復号できるようにする | ホストごとの鍵、共通鍵 1 つ、key 名をそのままファイル名にする |
| 2026-11-21 | store は public 必須。取得は無認証。認証を要求されたらエラー終了。private の運用は仕様の範囲外 | 読み手に認証を要求しない（ユーザー指示） | private/public を運用判断に任せる |
| 2026-11-21 | git は `git2`（libgit2 同梱）。`git` コマンドを使わない。受け皿の bare repo は実行ごとに一時作成して消し、ホストに何も残さない | `git` コマンドに頼らない（ユーザー指示）。任意の git repo で書き込みまで動く。純 Rust の `gix` は push が未実装（2026-11-21 に crate-status.md で確認） | 外部 `git` コマンド、`gix`、forge の REST API（forge ごとの実装になり「任意の git repo」を満たさない） |
| 2026-11-21 | `register` の入力は隠し入力・`--clipboard`・`--stdin`。`--stdin` では、エージェントのセッションが受け取った値も登録できる | ユーザー指示。値を引数で渡すとプロセス一覧に出る | 値をコマンドライン引数で渡す |
| 2026-11-21 | この repo の文書に、特定のネットワークの事情（store の場所、秘密の一覧と用途、ホスト名や IP、社内の registry や CI の実行環境、アカウント名、運用手順、既存の仕組みからの移行）を書かない | tokenbox は仕組みのみで、利用側のネットワークを知らない。規約（ユーザー指示） | 運用メモとして README に併記する（取り消し） |
