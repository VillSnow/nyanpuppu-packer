use crate::backend::{
    FetchProgress, Server, add_server, fetch_server_emojis, list_servers, rebuild_hash_parts,
    remove_server,
};
use dioxus::prelude::*;
use std::collections::HashMap;

#[component]
pub fn ServerManagement(
    mut servers: Signal<Vec<Server>>,
    mut message: Signal<String>,
    mut progress: Signal<HashMap<i64, FetchProgress>>,
) -> Element {
    let mut url = use_signal(String::new);
    let mut rebuilding_hash_parts = use_signal(|| false);
    let server_list = servers.read().clone();
    let progress_by_server = progress.read().clone();

    rsx! {
        section { class: "management-card",
            div { class: "management-card-header",
                div {
                    p { class: "management-eyebrow", "メンテナンス" }
                    h1 { "管理" }
                    p { class: "management-card-description",
                        "サーバーの登録と絵文字データの同期を管理します。"
                    }
                }
            }
            div { class: "management-card-section index-section",
                div { class: "management-section-heading",
                    div {
                        h2 { "ハッシュ検索インデックス" }
                        p {
                            "ハッシュの分割方式を変更した場合、保存済みデータの索引を再構築してください。"
                        }
                    }
                    button {
                        class: "secondary-button",
                        r#type: "button",
                        disabled: rebuilding_hash_parts(),
                        onclick: move |_| {
                            rebuilding_hash_parts.set(true);
                            message
                                .set(
                                    "ハッシュ検索インデックスを再構築しています…"
                                        .to_string(),
                                );
                            spawn(async move {
                                match rebuild_hash_parts().await {
                                    Ok(count) => {
                                        message
                                            .set(
                                                format!(
                                                    "ハッシュ検索インデックスを再構築しました（{count} 件）",
                                                ),
                                            )
                                    }
                                    Err(error) => {
                                        message.set(format!("再構築に失敗しました: {error}"))
                                    }
                                }
                                rebuilding_hash_parts.set(false);
                            });
                        },
                        if rebuilding_hash_parts() {
                            "再構築中…"
                        } else {
                            "検索インデックスを再構築"
                        }
                    }
                }
            }
            div { class: "management-card-section",
                div { class: "management-section-heading",
                    div {
                        h2 { "サーバー" }
                        p { "絵文字を取得するMisskeyサーバーを登録します。" }
                    }
                }
                form {
                    class: "server-register-form",
                    onsubmit: move |event| {
                        event.prevent_default();
                        let url_value = url().trim().to_string();
                        spawn(async move {
                            if url_value.is_empty() {
                                message.set("サーバーURLを入力してください".to_string());
                                return;
                            }
                            match add_server(url_value).await {
                                Ok(_) => {
                                    url.set(String::new());
                                    message.set("サーバーを登録しました".to_string());
                                    if let Ok(items) = list_servers().await {
                                        servers.set(items);
                                    }
                                }
                                Err(error) => message.set(error),
                            }
                        });
                    },
                    label { r#for: "instance-url", "サーバーURL" }
                    div { class: "server-register-controls",
                        input {
                            id: "instance-url",
                            r#type: "url",
                            required: true,
                            placeholder: "https://misskey.example.com",
                            value: "{url}",
                            oninput: move |event| url.set(event.value()),
                        }
                        button { class: "primary-button", r#type: "submit", "サーバーを登録" }
                    }
                }
                p { class: "management-status", role: "status", "{message}" }
            }
            div { class: "management-card-section registered-servers-section",
                div { class: "management-section-heading registered-servers-heading",
                    div {
                        h2 { "登録済みサーバー" }
                        p { "{server_list.len()} 件のサーバー" }
                    }
                }
                if server_list.is_empty() {
                    p { class: "empty-server-list",
                        "登録されているサーバーはありません。"
                    }
                } else {
                    ul { class: "server-list",
                        for server in server_list.clone() {
                            li { class: "server-list-item", key: "{server.id}",
                                div { class: "server-details",
                                    strong { class: "server-url", "{server.url}" }
                                    div { class: "server-meta",
                                        span { "絵文字 {server.emoji_count} 件" }
                                        if let Some(fetched_at) = &server.last_fetched_at {
                                            span { "最終取得: {fetched_at} UTC" }
                                        }
                                    }
                                }
                                div { class: "server-actions",
                                    button {
                                        class: "secondary-button",
                                        r#type: "button",
                                        disabled: progress_by_server.contains_key(&server.id),
                                        onclick: move |_| {
                                            let id = server.id;
                                            progress
                                                .with_mut(|updates| {
                                                    updates.insert(id, FetchProgress::listing(id));
                                                });
                                            spawn(async move {
                                                match fetch_server_emojis(id).await {
                                                    Ok(summary) => {
                                                        if let Ok(items) = list_servers().await {
                                                            servers.set(items);
                                                        }
                                                        progress
                                                            .with_mut(|updates| {
                                                                updates.remove(&id);
                                                            });
                                                        message
                                                            .set(
                                                                format!(
                                                                    "絵文字 {} 件を取得しました（画像 {} 件のハッシュを作成）",
                                                                    summary.emoji_count,
                                                                    summary.image_count,
                                                                ),
                                                            );
                                                    }
                                                    Err(error) => {
                                                        progress
                                                            .with_mut(|updates| {
                                                                updates.remove(&id);
                                                            });
                                                        message.set(error);
                                                    }
                                                }
                                            });
                                        },
                                        if progress_by_server.contains_key(&server.id) {
                                            "取得中…"
                                        } else {
                                            "絵文字を取得・更新"
                                        }
                                    }
                                    button {
                                        class: "text-danger-button",
                                        r#type: "button",
                                        onclick: move |_| {
                                            let id = server.id;
                                            spawn(async move {
                                                match remove_server(id).await {
                                                    Ok(_) => {
                                                        if let Ok(items) = list_servers().await {
                                                            servers.set(items);
                                                        }
                                                        message.set("サーバーを削除しました".to_string());
                                                    }
                                                    Err(error) => message.set(error),
                                                }
                                            });
                                        },
                                        "削除"
                                    }
                                }
                                if let Some(current) = progress_by_server.get(&server.id).cloned() {
                                    div { class: "fetch-progress",
                                        p {
                                            if current.stage == "listing" {
                                                "絵文字一覧を取得中…"
                                            } else if current.stage == "images" {
                                                "画像を処理中… {current.completed} / {current.total}"
                                            } else if current.stage == "saving" {
                                                "SQLiteへ保存中…"
                                            } else {
                                                "取得完了"
                                            }
                                        }
                                        progress {
                                            max: "100",
                                            value: "{current.percent()}",
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
}
