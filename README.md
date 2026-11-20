# tokenbox

共有の秘密（トークン・API キー）を、利用側が指定した git repo（store）に暗号化して置き、git URL と key 名を知る者が取り出すための CLI。tokenbox は仕組みだけを提供し、store の場所は引数 `<git-url>` で毎回渡す（`tokenbox <git-url> register <key-name>` / `tokenbox <git-url> get <key-name>`）。Rust 製、単一バイナリ、macOS / Linux / Windows。

- 仕様: [docs/SPEC.md](docs/SPEC.md)
- 作業手順（開発・配布）: [docs/WORKFLOW.md](docs/WORKFLOW.md)
- 配布設計（brew tap / apt / winget / npx）: [docs/DISTRIBUTION.md](docs/DISTRIBUTION.md)
- 決定の記録: [docs/DECISIONS.md](docs/DECISIONS.md)

## 文書の規約

この repo の文書には、**特定のネットワークの事情を書かない**。tokenbox は仕組みだけを提供し、利用側のネットワークを知らない。

書かないもの: store の場所（git URL・repo 名）、秘密の一覧と用途、key 名、ホストの名前や IP、社内の registry や CI の実行環境、アカウント名、運用手順、既存の仕組みからの移行手順。これらは利用側が自分の管理場所に置く。

## repo

| 場所 | 用途 |
|---|---|
| GitHub `unagents/tokenbox` | ソース。GitHub Actions で release・npm publish |
| GitHub `unagents/homebrew-tap` | `Formula/tokenbox.rb` |
| npm `@unagents/tokenbox` | `npx @unagents/tokenbox` 用ラッパー + プラットフォーム別バイナリ |
