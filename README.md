# Rust Common Libraries

## これはなに

Rustのプロジェクトで使用する共通ライブラリのコレクションです。

## 使い方

Cargo.tomlに以下を追加してください。

```toml
[dependencies]
common-lib = { git = "https://github.com/cffnpwr/rust-common-lib.git", features = ["<feature_name>"] }
```

`<feature_name>`には、使用したいライブラリの名前を指定してください。
デフォルトではどのライブラリも読み込まれません。

### 個別ライブラリの使用

ライブラリを個別に使用する場合は、以下のように指定してください。

```toml
[dependencies]
<library_name> = { git = "https://github.com/cffnpwr/rust-common-lib.git" }
```

例えば`patricia-tree`を使用する場合は、以下のように指定します。

```toml
[dependencies]
patricia-tree = { git = "https://github.com/cffnpwr/rust-common-lib.git" }
```

## ライブラリ一覧

- `patricia-tree`: Patricia Treeの実装
- `auto-impl-macro`: 複数のトレイトの実装に同一の実装を使用する際に使用するマクロ

## ライセンス

[MIT License](./LICENSE)
