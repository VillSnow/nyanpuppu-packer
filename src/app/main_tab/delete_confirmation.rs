use super::editing_emoji::EditingEmojis;
use dioxus::prelude::*;

#[component]
pub fn DeleteConfirmation(
    count: usize,
    mut pending_delete_count: Signal<Option<usize>>,
) -> Element {
    let mut emojis: EditingEmojis = use_context();

    rsx! {
        div { class: "confirm-overlay",
            section {
                class: "confirm-dialog",
                role: "dialog",
                aria_modal: "true",
                aria_label: "画像の削除確認",
                p { "{count}件の画像を表から削除します" }
                div { class: "confirm-dialog-actions",
                    button {
                        r#type: "button",
                        onclick: move |_| pending_delete_count.set(None),
                        "キャンセル"
                    }
                    button {
                        r#type: "button",
                        class: "danger-button",
                        onclick: move |_| {
                            emojis.with_mut(|all| all.retain(|emoji| !emoji.read().selected));
                            pending_delete_count.set(None);
                        },
                        "削除"
                    }
                }
            }
        }
    }
}
