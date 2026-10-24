#![allow(non_snake_case)]

mod delete_confirmation;
mod editing_emoji;
mod emojis_grid;
mod import_emojis;
mod matching;

use crate::backend::{ExportEmoji, export_emoji_package};
use delete_confirmation::DeleteConfirmation;
use dioxus::prelude::*;
use editing_emoji::EditingEmojis;
use emojis_grid::{ColumnVisibilities, EmojisGrid, use_column_visibilities};
use import_emojis::import_images;
use matching::{MatchingOverlay, MatchingState, rematch_emojis, use_matching_state};

/// 「パッケージ作成」タブ。画像の読み込み・照合・編集・書き出しと、それに伴う状態を保持する。
#[component]
pub fn MainTab(active: bool) -> Element {
    let emojis = use_context_provider(|| EditingEmojis::new(Vec::new()));
    let match_threshold = use_signal(|| 10u32);
    let matching = use_matching_state();
    let export_message = use_signal(String::new);
    let pending_delete_count = use_signal(|| None::<usize>);
    let visibilities = use_column_visibilities();

    let current_emojis = emojis.read().clone();
    let current_matching_progress = (matching.progress)();

    rsx! {
        div { class: "workspace-content main-tab", hidden: !active,
            ImageAddToolbar { match_threshold, matching }
            if current_emojis.is_empty() {
                section {
                    p { "画像を追加すると、照合結果がここに表示されます。" }
                }
            } else {
                section { class: "emojis-section",
                    EmojisToolbar {
                        export_message,
                        pending_delete_count,
                        visibilities,
                    }
                    EmojisGrid { visibilities }
                }
            }
        }
        if matching.is_active() {
            MatchingOverlay { progress: current_matching_progress }
        }
        if let Some(count) = pending_delete_count() {
            DeleteConfirmation { count, pending_delete_count }
        }
    }
}

#[component]
fn LabeledCheckbox(label: String, mut flag: Signal<bool>) -> Element {
    rsx! {
        label {
            input {
                r#type: "checkbox",
                checked: flag(),
                onchange: move |event| flag.set(event.checked()),
            }
            "{label}"
        }
    }
}

#[component]
fn ImageAddToolbar(match_threshold: Signal<u32>, matching: MatchingState) -> Element {
    let emojis: EditingEmojis = use_context();
    let mut threshold_change_id = use_signal(|| 0u64);

    let matched_emoji_count = emojis
        .read()
        .iter()
        .filter(|emoji| !emoji.read().matches.is_empty())
        .count();
    let emoji_count = emojis.read().len();

    rsx! {
        section { class: "image-add-toolbar",
            div { class: "image-toolbar-primary",
                label { class: "image-file-picker",
                    "画像を追加"
                    input {
                        id: "image-input",
                        r#type: "file",
                        accept: "image/*",
                        multiple: true,
                        onchange: move |event| import_images(event.files(), emojis, match_threshold, matching),
                    }
                }
            }
            div { class: "image-toolbar-settings",
                label { r#for: "match-threshold",
                    "照合閾値"
                    span { class: "match-threshold-value", "{match_threshold}" }
                }
                input {
                    id: "match-threshold",
                    r#type: "range",
                    min: "0",
                    max: "20",
                    step: "1",
                    value: "{match_threshold}",
                    aria_label: "画像照合の閾値",
                    oninput: move |event| {
                        if let Ok(value) = event.value().parse::<u32>() {
                            match_threshold.set(value);
                            threshold_change_id.set(threshold_change_id() + 1);
                            let current_change_id = threshold_change_id();
                            spawn(async move {
                                gloo_timers::future::TimeoutFuture::new(2000).await;
                                if threshold_change_id() == current_change_id {
                                    rematch_emojis(emojis, match_threshold, value, matching);
                                }
                            });
                        }
                    },
                }
            }
            p { class: "emoji-match-status",
                "一致あり: {matched_emoji_count} / {emoji_count} 枚"
            }
        }
    }
}

#[component]
fn EmojisToolbar(
    mut export_message: Signal<String>,
    mut pending_delete_count: Signal<Option<usize>>,
    visibilities: ColumnVisibilities,
) -> Element {
    let emojis: EditingEmojis = use_context();
    let selected_count = emojis
        .read()
        .iter()
        .filter(|emoji| emoji.read().selected)
        .count();
    let mut exporting = use_signal(|| false);

    rsx! {
        div { class: "emojis-toolbar",
            p { "選択中: {selected_count} 件" }
            div { class: "emojis-toolbar-actions",
                div { class: "column-visibility-controls",
                    LabeledCheckbox {
                        label: "ファイル名を表示",
                        flag: visibilities.file_name,
                    }
                    LabeledCheckbox {
                        label: "カテゴリを表示",
                        flag: visibilities.category_fields,
                    }
                    LabeledCheckbox {
                        label: "絵文字名を表示",
                        flag: visibilities.emoji_name_fields,
                    }
                    LabeledCheckbox {
                        label: "タグを表示",
                        flag: visibilities.alias_fields,
                    }
                    LabeledCheckbox {
                        label: "ライセンスを表示",
                        flag: visibilities.license_fields,
                    }
                    LabeledCheckbox {
                        label: "候補を表示",
                        flag: visibilities.candidates,
                    }
                }
                button {
                    class: "emojis-toolbar-danger-button",
                    r#type: "button",
                    disabled: selected_count == 0,
                    onclick: move |_| pending_delete_count.set(Some(selected_count)),
                    "選択中の画像を削除"
                }
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
                    "選択中の画像を書き出し"
                }
            }
            if !export_message.read().is_empty() {
                p { style: "white-space: pre-line;", "{export_message}" }
            }
        }
    }
}
