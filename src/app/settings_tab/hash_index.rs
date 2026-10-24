use crate::backend::rebuild_hash_parts;
use dioxus::prelude::*;

#[component]
pub fn HashIndexRebuild() -> Element {
    let mut rebuilding = use_signal(|| false);
    let mut message = use_signal(String::new);

    rsx! {
        div { class: "hash-index-rebuild",
            p {
                "ハッシュの分割方式を変更した場合、保存済みデータの索引を再構築してください。"
            }
            div { class: "hash-index-rebuild-actions",
                button {
                    class: "secondary-button",
                    r#type: "button",
                    disabled: rebuilding(),
                    onclick: move |_| {
                        rebuilding.set(true);
                        message.set("ハッシュ検索インデックスを再構築しています…".to_string());
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
                            rebuilding.set(false);
                        });
                    },
                    if rebuilding() {
                        "再構築中…"
                    } else {
                        "検索インデックスを再構築"
                    }
                }
            }
            p { class: "management-status", role: "status", "{message}" }
        }
    }
}
