#![allow(non_snake_case)]

mod main_tab;
mod settings_tab;

use dioxus::prelude::*;
use main_tab::MainTab;
use settings_tab::SettingsTab;

static CSS: Asset = asset!("/assets/styles.css");
static EMOJIS_GRID_CSS: Asset = asset!("/assets/emojis-grid.css");

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Main,
    Settings,
}

/// タブの切り替えのみを担当する。各タブは常にマウントされ、自身の状態を保持する。
pub fn App() -> Element {
    let mut current_tab = use_signal(|| Tab::Main);

    rsx! {
        link { rel: "stylesheet", href: CSS }
        link { rel: "stylesheet", href: EMOJIS_GRID_CSS }

        div { class: "app-shell",
            nav { class: "tab-bar", aria_label: "ページ",
                TabButton {
                    label: "パッケージ作成",
                    tab: Tab::Main,
                    current_tab: current_tab(),
                    onselect: move |tab| current_tab.set(tab),
                }
                TabButton {
                    label: "管理",
                    tab: Tab::Settings,
                    current_tab: current_tab(),
                    onselect: move |tab| current_tab.set(tab),
                }
            }
            main { class: "workspace",
                MainTab { active: current_tab() == Tab::Main }
                SettingsTab { active: current_tab() == Tab::Settings }
            }
        }
    }
}

#[component]
fn TabButton(
    label: &'static str,
    tab: Tab,
    current_tab: Tab,
    onselect: EventHandler<Tab>,
) -> Element {
    let active = tab == current_tab;

    rsx! {
        button {
            class: if active { "tab-button active" } else { "tab-button" },
            r#type: "button",
            aria_current: if active { "page" } else { "false" },
            onclick: move |_| onselect.call(tab),
            "{label}"
        }
    }
}
