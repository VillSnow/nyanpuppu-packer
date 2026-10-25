#![allow(non_snake_case)]

use dioxus::prelude::*;

/// 「ヘルプ」タブ。操作が分かりにくい点だけを短く説明する。
#[component]
pub fn HelpTab(active: bool) -> Element {
    rsx! {
        div { class: "workspace-content help-tab", hidden: !active,
            div { class: "help-tab-content",
                h1 { "使い方" }
                p { class: "help-lead",
                    "手元の画像に似た絵文字を Misskey サーバーから探し、名前やタグなどを引き継いで、絵文字パッケージ（ZIP）にまとめます。"
                }

                section {
                    h2 { "流れ" }
                    ol {
                        li {
                            b { "管理" }
                            "：サーバーを登録して「絵文字を取得・更新」を押す"
                        }
                        li {
                            b { "編集" }
                            "：画像を追加し、候補から名前やタグを選ぶ"
                        }
                        li {
                            b { "書き出し" }
                            "：確認して ZIP を保存する"
                        }
                    }
                }

                section {
                    h2 { "ポイント" }
                    ul {
                        li {
                            "最初に「管理」で絵文字を取得しておかないと、候補は出ません。"
                        }
                        li {
                            "書き出しは、チェックの有無に関わらず編集中のすべてが対象です。"
                        }
                        li { "書き出した ZIP は Misskey のカスタム絵文字インポートで使えます。" }
                    }
                }
            }
        }
    }
}
