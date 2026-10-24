#![allow(non_snake_case)]

use super::edit_tab::EditingEmojis;
use crate::backend::{
    EmojiMatch, ExportEmoji, Server, ServerEmoji, export_emoji_package, list_server_emojis,
    list_servers, match_hash,
};
use dioxus::prelude::*;
use std::collections::HashMap;

/// 比較先サーバーの絵文字 1 件分の表示用データ。未設定の項目は空文字で持つ。
#[derive(Clone, PartialEq)]
struct ServerRow {
    name: String,
    category: String,
    aliases: Vec<String>,
    license: String,
    image_url: Option<String>,
}

impl ServerRow {
    fn from_server(emoji: &ServerEmoji) -> Self {
        Self {
            name: emoji.name.clone(),
            category: trimmed(&emoji.category),
            aliases: emoji.aliases.clone(),
            license: trimmed(&emoji.license),
            image_url: emoji.image_url.clone(),
        }
    }

    fn from_match(found: &EmojiMatch, image_url: Option<String>) -> Self {
        Self {
            name: found.name.clone(),
            category: trimmed(&found.category),
            aliases: found.aliases.clone(),
            license: trimmed(&found.license),
            image_url,
        }
    }
}

fn trimmed(value: &Option<String>) -> String {
    value.as_deref().map(str::trim).unwrap_or("").to_string()
}

fn sorted_aliases(aliases: &[String]) -> Vec<&str> {
    let mut aliases = aliases.iter().map(|alias| alias.trim()).collect::<Vec<_>>();
    aliases.sort_unstable();
    aliases
}

/// 絵文字名が同じもの、続けて知覚ハッシュが近いものの順に、比較先サーバーの絵文字を集める。
fn server_rows(
    emoji_name: &str,
    hash_matches: &[EmojiMatch],
    server_emojis: &HashMap<String, ServerEmoji>,
) -> Vec<ServerRow> {
    let name = emoji_name.trim();
    let mut rows = Vec::new();
    if let Some(same_name) = server_emojis.get(name).filter(|_| !name.is_empty()) {
        rows.push(ServerRow::from_server(same_name));
    }
    for found in hash_matches {
        if found.name == name {
            continue;
        }
        let image_url = server_emojis
            .get(&found.name)
            .and_then(|server| server.image_url.clone());
        rows.push(ServerRow::from_match(found, image_url));
    }
    rows
}

/// 編集中のすべての絵文字（チェック状態に関わらず）の編集内容を一覧表示する表。
/// 比較先サーバーに同名または知覚ハッシュの近い絵文字があれば、その直下に行として表示する。
#[component]
fn ReviewTable(
    comparing: bool,
    pending: bool,
    only_matched: bool,
    server_emojis: HashMap<String, ServerEmoji>,
    hash_matches: HashMap<String, Vec<EmojiMatch>>,
) -> Element {
    let emojis: EditingEmojis = use_context();
    let all_count = emojis.read().len();
    let targets = emojis
        .read()
        .iter()
        .map(|emoji| emoji.read().clone())
        .map(|emoji| {
            let rows = if comparing {
                let matches = hash_matches
                    .get(&emoji.md5)
                    .map(Vec::as_slice)
                    .unwrap_or_default();
                server_rows(&emoji.emoji_name, matches, &server_emojis)
            } else {
                Vec::new()
            };
            (emoji, rows)
        })
        .filter(|(_, rows)| !(comparing && only_matched) || !rows.is_empty())
        .collect::<Vec<_>>();

    if all_count == 0 {
        return rsx! {
            p { "編集タブで画像を追加すると、ここに一覧が表示されます。" }
        };
    }
    if targets.is_empty() {
        return rsx! {
            p {
                if pending {
                    "比較先サーバーと照合しています…"
                } else {
                    "比較先サーバーに同名または近い絵文字がある項目はありません。"
                }
            }
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
            for (emoji , rows) in targets {
                tbody { key: "{emoji.md5}",
                    tr {
                        td {
                            img { src: "{emoji.preview}", alt: "" }
                        }
                        td { "{emoji.file_name}" }
                        td {
                            {cell_text(&emoji.emoji_name)}
                            if comparing && !pending && rows.is_empty() {
                                div { class: "empty-value", "サーバーに同名・近い絵文字なし" }
                            }
                        }
                        td { {cell_text(&emoji.category)} }
                        td { {cell_text(&emoji.aliases.join(", "))} }
                        td { {cell_text(&emoji.license)} }
                    }
                    for row in rows {
                        {
                            let category_differs = row.category != emoji.category.trim();
                            let aliases_differs = sorted_aliases(&row.aliases)
                                != sorted_aliases(&emoji.aliases);
                            let license_differs = row.license != emoji.license.trim();
                            rsx! {
                                tr { key: "{row.name}", class: "server-row", title: "サーバーの値",
                                    td {
                                        if let Some(image_url) = &row.image_url {
                                            img { src: "{image_url}", alt: "" }
                                        }
                                    }
                                    td {}
                                    td { {cell_text(&row.name)} }
                                    td { class: if category_differs { "differs" },
                                        {cell_text(&row.category)}
                                    }
                                    td { class: if aliases_differs { "differs" },
                                        {cell_text(&row.aliases.join(", "))}
                                    }
                                    td { class: if license_differs { "differs" },
                                        {cell_text(&row.license)}
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

/// 照合の入力。これが変わったときだけ照合し直す。
#[derive(Clone, PartialEq)]
struct MatchKey {
    host: String,
    threshold: u32,
    hashes: Vec<(String, String)>,
}

/// 比較先サーバーに絞り込んだ照合結果（md5 ごと）。
#[derive(Clone, Default, PartialEq)]
struct HashMatchResult {
    host: String,
    matches: HashMap<String, Vec<EmojiMatch>>,
    error: Option<String>,
}

/// 「書き出し」タブ。編集中のすべての絵文字をパッケージとして書き出す。
#[component]
pub fn ExportTab(active: bool) -> Element {
    let emojis: EditingEmojis = use_context();
    // 編集タブとは独立した、比較先サーバーとの照合閾値。スライダー操作が止まってから反映する
    let mut threshold_input = use_signal(|| 10u32);
    let mut threshold = use_signal(|| 10u32);
    let mut threshold_change_id = use_signal(|| 0u64);
    let mut exporting = use_signal(|| false);
    let mut export_message = use_signal(String::new);
    let emoji_count = emojis.read().len();
    let mut servers = use_signal(Vec::<Server>::new);
    let mut compare_host = use_signal(String::new);
    let mut only_matched = use_signal(|| false);
    let mut tab_active = use_signal(|| active);
    let mut hash_result = use_signal(HashMatchResult::default);
    let mut requested_key = use_signal(|| None::<MatchKey>);

    // 管理タブでの登録変更を反映するため、タブを開くたびにサーバー一覧を取得し直す
    use_effect(use_reactive!(|active| {
        tab_active.set(active);
        if active {
            spawn(async move {
                if let Ok(items) = list_servers().await {
                    servers.set(items);
                }
            });
        }
    }));

    // 文字列の編集などでは変化しないよう、照合に必要な (md5, ハッシュ) だけをメモ化する
    let hashes = use_memo(move || {
        emojis
            .read()
            .iter()
            .filter_map(|emoji| {
                let emoji = emoji.read();
                emoji.hash.clone().map(|hash| (emoji.md5.clone(), hash))
            })
            .collect::<Vec<_>>()
    });

    // 比較先・閾値・画像が変わったときに、バックグラウンドで照合し直して完了時に表を更新する
    use_effect(move || {
        let key = MatchKey {
            host: compare_host(),
            threshold: threshold(),
            hashes: hashes(),
        };
        if !tab_active() || key.host.is_empty() || requested_key.peek().as_ref() == Some(&key) {
            return;
        }
        requested_key.set(Some(key.clone()));
        spawn(async move {
            let mut matches = HashMap::new();
            let mut error = None;
            for (md5, hash) in &key.hashes {
                if requested_key.peek().as_ref() != Some(&key) {
                    return;
                }
                match match_hash(hash.clone(), key.threshold).await {
                    Ok(found) => {
                        let found = found
                            .into_iter()
                            .filter(|item| item.host == key.host)
                            .collect::<Vec<_>>();
                        if !found.is_empty() {
                            matches.insert(md5.clone(), found);
                        }
                    }
                    Err(failure) => {
                        error.get_or_insert(failure);
                    }
                }
                gloo_timers::future::TimeoutFuture::new(0).await;
            }
            if requested_key.peek().as_ref() == Some(&key) {
                hash_result.set(HashMatchResult {
                    host: key.host.clone(),
                    matches,
                    error,
                });
            }
        });
    });

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
    let mut emojis_loaded = false;
    if !selected_host.is_empty()
        && let Some((host, result)) = &*server_emojis.read()
        && *host == selected_host
    {
        emojis_loaded = true;
        match result {
            Ok(items) => loaded_server_emojis = items.clone(),
            Err(error) => compare_error = error.clone(),
        }
    }
    let mut loaded_hash_matches = HashMap::new();
    let mut hashes_loaded = false;
    if !selected_host.is_empty() && hash_result.read().host == selected_host {
        hashes_loaded = true;
        loaded_hash_matches = hash_result.read().matches.clone();
        if compare_error.is_empty() {
            compare_error = hash_result.read().error.clone().unwrap_or_default();
        }
    }
    let pending = !selected_host.is_empty() && !(emojis_loaded && hashes_loaded);

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
                            option { value: "{server.url}", "{server.display_name()}" }
                        }
                    }
                }
                label { class: "export-compare-select",
                    span { "照合閾値" }
                    input {
                        r#type: "range",
                        min: "0",
                        max: "20",
                        step: "1",
                        value: "{threshold_input}",
                        disabled: selected_host.is_empty(),
                        aria_label: "比較先サーバーとの照合閾値",
                        oninput: move |event| {
                            if let Ok(value) = event.value().parse::<u32>() {
                                threshold_input.set(value);
                                threshold_change_id += 1;
                                let current_change_id = threshold_change_id();
                                spawn(async move {
                                    gloo_timers::future::TimeoutFuture::new(1000).await;
                                    if threshold_change_id() == current_change_id {
                                        threshold.set(value);
                                    }
                                });
                            }
                        },
                    }
                    span { "{threshold_input}" }
                }
                label { class: "export-compare-select",
                    input {
                        r#type: "checkbox",
                        checked: only_matched() && !selected_host.is_empty(),
                        disabled: selected_host.is_empty(),
                        onchange: move |event| only_matched.set(event.checked()),
                    }
                    span { "比較先に同名または近い絵文字があるものだけ表示" }
                }
                if !compare_error.is_empty() {
                    p { class: "export-compare-error", "{compare_error}" }
                }
                if !selected_host.is_empty() {
                    p { class: "export-compare-legend",
                        "背景色付きの行がサーバーの値です。"
                        span { class: "legend-differs", "橙色" }
                        "のセルは編集結果と異なる値です。"
                        if pending {
                            " 照合しています…"
                        }
                    }
                }
                ReviewTable {
                    comparing: !selected_host.is_empty(),
                    pending,
                    only_matched: only_matched(),
                    server_emojis: loaded_server_emojis,
                    hash_matches: loaded_hash_matches,
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
