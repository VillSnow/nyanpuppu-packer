#![allow(non_snake_case)]

mod image_compare;
mod server_management;

use crate::backend::{FetchProgress, Server, list_servers, subscribe_fetch_progress};
use dioxus::prelude::*;
use image_compare::ImageCompare;
use server_management::ServerManagement;
use std::collections::HashMap;

/// 「管理」タブ。サーバー登録・絵文字同期・画像比較ツールと、それに伴う状態を保持する。
#[component]
pub fn SettingsTab(active: bool) -> Element {
    let mut servers = use_signal(Vec::<Server>::new);
    let mut server_message = use_signal(String::new);
    let server_progress = use_signal(HashMap::<i64, FetchProgress>::new);

    use_future(move || async move {
        match list_servers().await {
            Ok(items) => servers.set(items),
            Err(error) => server_message.set(error),
        }
    });
    use_future(move || async move {
        subscribe_fetch_progress(server_progress).await;
    });

    rsx! {
        div { class: "workspace-content settings-tab", hidden: !active,
            div { class: "settings-tab-content",
                ServerManagement {
                    servers,
                    message: server_message,
                    progress: server_progress,
                }
                ImageCompare {}
            }
        }
    }
}
