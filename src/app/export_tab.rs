#![allow(non_snake_case)]

use super::edit_tab::EditingEmojis;
use crate::backend::{ExportEmoji, export_emoji_package};
use dioxus::prelude::*;

/// 「書き出し」タブ。編集タブで選択中の絵文字をパッケージとして書き出す。
#[component]
pub fn ExportTab(active: bool) -> Element {
    let emojis: EditingEmojis = use_context();
    let mut exporting = use_signal(|| false);
    let mut export_message = use_signal(String::new);
    let selected_count = emojis
        .read()
        .iter()
        .filter(|emoji| emoji.read().selected)
        .count();

    rsx! {
        div { class: "workspace-content export-tab", hidden: !active,
            section {
                p { class: "emojis-toolbar-count", "書き出し対象: {selected_count} 件" }
                div { class: "emojis-toolbar-actions",
                    button {
                        class: "emojis-toolbar-export-button",
                        r#type: "button",
                        disabled: selected_count == 0 || exporting(),
                        onclick: move |_| {
                            let export_emojis = emojis
                                .read()
                                .iter()
                                .filter(|emoji| emoji.read().selected)
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
