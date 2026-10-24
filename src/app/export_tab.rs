#![allow(non_snake_case)]

use super::edit_tab::EditingEmojis;
use crate::backend::{
    ExportEmoji, Server, ServerEmoji, export_emoji_package, list_server_emojis, list_servers,
};
use dioxus::prelude::*;
use std::collections::HashMap;

/// 未設定（None）と空文字を同一視して比較する。
fn normalized(value: &Option<String>) -> &str {
    value.as_deref().map(str::trim).unwrap_or("")
}

fn sorted_aliases(aliases: &[String]) -> Vec<&str> {
    let mut aliases = aliases.iter().map(|alias| alias.trim()).collect::<Vec<_>>();
    aliases.sort_unstable();
    aliases
}

/// 編集中のすべての絵文字（チェック状態に関わらず）の編集内容を一覧表示する表。
/// 比較先サーバーに同名の絵文字があれば、その直下にサーバーでの値を行として表示する。
#[component]
fn ReviewTable(
    comparing: bool,
    only_matched: bool,
    server_emojis: HashMap<String, ServerEmoji>,
) -> Element {
    let emojis: EditingEmojis = use_context();
    let all_count = emojis.read().len();
    let targets = emojis
        .read()
        .iter()
        .map(|emoji| emoji.read().clone())
        .filter(|emoji| {
            !(comparing && only_matched) || server_emojis.contains_key(emoji.emoji_name.trim())
        })
        .collect::<Vec<_>>();

    if all_count == 0 {
        return rsx! {
            p { "編集タブで画像を追加すると、ここに一覧が表示されます。" }
        };
    }
    if targets.is_empty() {
        return rsx! {
            p { "比較先サーバーに同名の絵文字がある項目はありません。" }
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
            // 区切り線を絵文字単位で引くため、絵文字ごとに tbody を分ける
            for emoji in targets {
                {
                    let server = server_emojis.get(emoji.emoji_name.trim());
                    let server_aliases = server.map(|s| s.aliases.join(", ")).unwrap_or_default();
                    let category_differs = server
                        .is_some_and(|s| normalized(&s.category) != emoji.category.trim());
                    let aliases_differs = server
                        .is_some_and(|s| sorted_aliases(&s.aliases) != sorted_aliases(&emoji.aliases));
                    let license_differs = server
                        .is_some_and(|s| normalized(&s.license) != emoji.license.trim());
                    rsx! {
                        tbody { key: "{emoji.md5}",
                            tr {
                                td {
                                    img { src: "{emoji.preview}", alt: "" }
                                }
                                td { "{emoji.file_name}" }
                                td {
                                    {cell_text(&emoji.emoji_name)}
                                    if comparing && server.is_none() && !emoji.emoji_name.trim().is_empty() {
                                        div { class: "empty-value", "サーバーに同名の絵文字なし" }
                                    }
                                }
                                td { {cell_text(&emoji.category)} }
                                td { {cell_text(&emoji.aliases.join(", "))} }
                                td { {cell_text(&emoji.license)} }
                            }
                            if let Some(server) = server {
                                tr { class: "server-row", title: "サーバーの値",
                                    td {
                                        if let Some(image_url) = &server.image_url {
                                            img { src: "{image_url}", alt: "" }
                                        }
                                    }
                                    td {}
                                    td { {cell_text(&server.name)} }
                                    td { class: if category_differs { "differs" },
                                        {cell_text(normalized(&server.category))}
                                    }
                                    td { class: if aliases_differs { "differs" },
                                        {cell_text(&server_aliases)}
                                    }
                                    td { class: if license_differs { "differs" },
                                        {cell_text(normalized(&server.license))}
                                    }
                                }
                            }
                        }
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
    let mut servers = use_signal(Vec::<Server>::new);
    let mut compare_host = use_signal(String::new);
    let mut only_matched = use_signal(|| false);

    // 管理タブでの登録変更を反映するため、タブを開くたびにサーバー一覧を取得し直す
    use_effect(use_reactive!(|active| {
        if active {
            spawn(async move {
                if let Ok(items) = list_servers().await {
                    servers.set(items);
                }
            });
        }
    }));

    let server_emojis = use_resource(move || async move {
        let host = compare_host();
        if host.is_empty() {
            return (host, Ok(HashMap::new()));
        }
        let result = list_server_emojis(host.clone()).await.map(|items| {
            items
                .into_iter()
                .map(|item| (item.name.clone(), item))
                .collect::<HashMap<_, _>>()
        });
        (host, result)
    });

    // 登録解除されたサーバーが選択されたままにならないようにする
    let selected_host = if servers.read().iter().any(|s| s.url == compare_host()) {
        compare_host()
    } else {
        String::new()
    };

    // 取得し直している間に古い比較先の結果を使わないよう、ホストが一致する結果のみ採用する
    let mut loaded_server_emojis = HashMap::new();
    let mut compare_error = String::new();
    if !selected_host.is_empty()
        && let Some((host, result)) = &*server_emojis.read()
        && *host == selected_host
    {
        match result {
            Ok(items) => loaded_server_emojis = items.clone(),
            Err(error) => compare_error = error.clone(),
        }
    }

    rsx! {
        div { class: "workspace-content export-tab", hidden: !active,
            section {
                p { class: "emojis-toolbar-count", "書き出し対象: {emoji_count} 件" }
                label { class: "export-compare-select",
                    span { "比較先サーバー" }
                    select {
                        value: "{selected_host}",
                        onchange: move |event| compare_host.set(event.value()),
                        option { value: "", "比較しない" }
                        for server in servers.read().iter() {
                            option { value: "{server.url}", "{server.url}" }
                        }
                    }
                }
                label { class: "export-compare-select",
                    input {
                        r#type: "checkbox",
                        checked: only_matched() && !selected_host.is_empty(),
                        disabled: selected_host.is_empty(),
                        onchange: move |event| only_matched.set(event.checked()),
                    }
                    span { "比較先に同名の絵文字があるものだけ表示" }
                }
                if !compare_error.is_empty() {
                    p { class: "export-compare-error", "{compare_error}" }
                }
                if !selected_host.is_empty() {
                    p { class: "export-compare-legend",
                        "背景色付きの行がサーバーの値です。"
                        span { class: "legend-differs", "橙色" }
                        "のセルは編集結果と異なる値です。"
                    }
                }
                ReviewTable {
                    comparing: !selected_host.is_empty(),
                    only_matched: only_matched(),
                    server_emojis: loaded_server_emojis,
                }
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
