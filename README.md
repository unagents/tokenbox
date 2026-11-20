# tokenbox

共有の秘密（トークン・API キー）を、利用側が指定した git repo（store）に暗号化して置き、git URL と key 名を知る者が取り出すための CLI。Rust 製、単一バイナリ。

## インストール

macOS（Homebrew）:

```
brew install unagents/tap/tokenbox
```

Linux（x86_64 / aarch64、glibc 2.31 以上）:

```
V=0.1.1; T=$(uname -m)-unknown-linux-gnu
curl -fsSL -o /tmp/tb.tgz https://github.com/unagents/tokenbox/releases/download/v$V/tokenbox-v$V-$T.tar.gz
curl -fsSL https://github.com/unagents/tokenbox/releases/download/v$V/SHA256SUMS | grep "tokenbox-v$V-$T.tar.gz" | sed 's#  .*#  /tmp/tb.tgz#' | sha256sum -c -
sudo tar -xzf /tmp/tb.tgz -C /usr/local/bin tokenbox && rm /tmp/tb.tgz
```

## 使い方

```
tokenbox <git-url> register <key-name>          # 隠し入力。--clipboard / --stdin も可
tokenbox <git-url> get <key-name>               # 値を stdout に出す
```

- store（`<git-url>` の repo）は public であること。取得は無認証で行う。
- `register` は push するため、ホストの git 設定に store への書き込みの認証が要る。
- 読めるのは git URL と key 名の両方を知る者。key 名には長いランダムな部分を付ける。
- store の場所も key 名も、tokenbox は保存しない。毎回引数で渡す。

## 文書の規約

この repo にはコードだけを置く（実装、テスト、ビルドと release の定義、この README）。

次のものは、この repo に置かない。

- 仕様・設計・決定・進捗などの文書。
- 特定のネットワークの事情（store の場所、秘密の一覧と用途、key 名、ホストの名前や IP、社内の registry や CI の実行環境、アカウント名、運用手順）。
