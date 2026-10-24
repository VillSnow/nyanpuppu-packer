use super::editing_emoji::EditingEmojis;
use dioxus::{html::FileData, prelude::*};

/// ZIP の読み込みで編集中の絵文字がすべて破棄されることを警告し、続行するか確認する。
/// `pending_zip` が `Some` の間だけ表示する。
/// 読み込み処理はダイアログが閉じても継続する必要があるため、`on_confirm` で呼び出し側に委ねる。
#[component]
pub fn ReplaceConfirmation(
    mut pending_zip: Signal<Option<FileData>>,
    on_confirm: Callback<FileData>,
) -> Element {
    let emojis: EditingEmojis = use_context();
    let Some(file) = pending_zip() else {
        return rsx! {};
    };
    let file_name = file.name();
    let count = emojis.read().len();

    rsx! {
        div { class: "confirm-overlay",
            section {
                class: "confirm-dialog",
                role: "dialog",
                aria_modal: "true",
                aria_label: "ZIP 読み込みの確認",
                p {
                    "{file_name} を読み込むと、追加済みの{count}件の画像と編集内容は破棄されます。"
                }
                p { "読み込みを続けますか？" }
                div { class: "confirm-dialog-actions",
                    button {
                        r#type: "button",
                        onclick: move |_| pending_zip.set(None),
                        "キャンセル"
                    }
                    button {
                        r#type: "button",
                        class: "danger-button",
                        onclick: move |_| {
                            pending_zip.set(None);
                            on_confirm.call(file.clone());
                        },
                        "破棄して読み込む"
                    }
                }
            }
        }
    }
}
