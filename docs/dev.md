# Tauri + Dioxus

This template should help get you started developing with Tauri and Dioxus.

## アプリのバージョン変更

Nushell から次のように実行すると、ルートと Tauri の Cargo.toml、および Tauri 設定のバージョンをまとめて更新できます。

```nu
nu set-version.nu 1.2.3
```

SemVer 形式のバージョンを指定してください（例: `1.2.3`, `1.2.3-beta.1`）。

## リリースタグの作成

バージョン変更をコミットした後、`v` を付けたタグを作成して push すると、GitHub Actions が各 OS 向けのビルドを行い、リリースのドラフトを作成します。

```sh
git tag v1.2.3
git push origin v1.2.3
```

## Recommended IDE Setup

[VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer) + [Dioxus](https://marketplace.visualstudio.com/items?itemName=DioxusLabs.dioxus).
