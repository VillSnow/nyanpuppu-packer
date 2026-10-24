#![allow(non_snake_case)]

use super::edit_tab::EditingEmojis;
use crate::backend::{ExportEmoji, export_emoji_package};
use dioxus::prelude::*;

/// 編集中のすべての絵文字（チェック状態に関わらず）の編集内容を一覧表示する表。
#[component]
fn ReviewTable() -> Element {
    let emojis: EditingEmojis = use_context();
    let targets = emojis
        .read()
        .iter()
        .map(|emoji| emoji.read().clone())
        .collect::<Vec<_>>();

    if targets.is_empty() {
        return rsx! {
            p { "編集タブで画像を追加すると、ここに一覧が表示されます。" }
        };
    }

    rsx! {
        table { class: "export-review-table",
            thead {
                tr {
                    th { "プレビュー" }
                    th { "ファイル名" }
                    th { "絵文字名" }
                    th { "カテゴリ" }
                    th { "タグ" }
                    th { "ライセンス" }
                }
            }
            tbody {
                for emoji in targets {
                    tr { key: "{emoji.md5}",
                        td {
                            img { src: "{emoji.preview}", alt: "" }
                        }
                        td { "{emoji.file_name}" }
                        td { {cell_text(&emoji.emoji_name)} }
                        td { {cell_text(&emoji.category)} }
                        td { {cell_text(&emoji.aliases.join(", "))} }
                        td { {cell_text(&emoji.license)} }
                    }
                }
            }
        }
    }
}

/// 未入力の項目はプレースホルダー表示にする。
fn cell_text(value: &str) -> Element {
    if value.is_empty() {
        rsx! {
            span { class: "empty-value", "未入力" }
        }
    } else {
        rsx! { "{value}" }
    }
}

/// 「書き出し」タブ。編集中のすべての絵文字をパッケージとして書き出す。
#[component]
pub fn ExportTab(active: bool) -> Element {
    let emojis: EditingEmojis = use_context();
    let mut exporting = use_signal(|| false);
    let mut export_message = use_signal(String::new);
    let emoji_count = emojis.read().len();

    rsx! {
        div { class: "workspace-content export-tab", hidden: !active,
            section {
                p { class: "emojis-toolbar-count", "書き出し対象: {emoji_count} 件" }
                ReviewTable {}
                div { class: "emojis-toolbar-actions",
                    button {
                        class: "emojis-toolbar-export-button",
                        r#type: "button",
                        disabled: emoji_count == 0 || exporting(),
                        onclick: move |_| {
                            let export_emojis = emojis
                                .read()
                                .iter()
                                .map(|emoji| ExportEmoji {
                                    file_name: emoji.read().file_name.clone(),
                                    emoji_name: emoji.read().emoji_name.clone(),
                                    category: emoji.read().category.clone(),
                                    aliases: emoji.read().aliases.clone(),
                                    license: emoji.read().license.clone(),
                                    image: emoji.read().bytes.as_ref().clone(),
                                })
                                .collect::<Vec<_>>();
                            exporting.set(true);
                            export_message.set("書き出しています…".to_string());
                            spawn(async move {
                                match export_emoji_package(export_emojis).await {
                                    Ok(Some(summary)) => {
                                        export_message
                                            .set(
                                                format!(
                                                    "{} 件を書き出しました: {}",
                                                    summary.emoji_count,
                                                    summary.path,
                                                ),
                                            )
                                    }
                                    Ok(None) => export_message.set(String::new()),
                                    Err(error) => export_message.set(error),
                                }
                                exporting.set(false);
                            });
                        },
                        "書き出し"
                    }
                }
                if !export_message.read().is_empty() {
                    p { style: "white-space: pre-line;", "{export_message}" }
                }
            }
        }
    }
}
