#![allow(non_snake_case)]

mod edit_tab;
mod export_tab;
mod help_tab;
mod settings_tab;

use dioxus::prelude::*;
use edit_tab::{EditTab, EditingEmojis};
use export_tab::ExportTab;
use help_tab::HelpTab;
use settings_tab::SettingsTab;

static CSS: Asset = asset!("/assets/styles.css");
static EMOJIS_GRID_CSS: Asset = asset!("/assets/emojis-grid.css");

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Edit,
    Export,
    Settings,
    Help,
}

/// タブの切り替えと、タブ間で共有する編集中の絵文字リストの提供を担当する。各タブは常にマウントされ、自身の状態を保持する。
pub fn App() -> Element {
    let mut current_tab = use_signal(|| Tab::Edit);
    use_context_provider(|| EditingEmojis::new(Vec::new()));

    rsx! {
        link { rel: "stylesheet", href: CSS }
        link { rel: "stylesheet", href: EMOJIS_GRID_CSS }

        div { class: "app-shell",
            nav { class: "tab-bar", aria_label: "ページ",
                TabButton {
                    label: "編集",
                    tab: Tab::Edit,
                    current_tab: current_tab(),
                    onselect: move |tab| current_tab.set(tab),
                }
                TabButton {
                    label: "書き出し",
                    tab: Tab::Export,
                    current_tab: current_tab(),
                    onselect: move |tab| current_tab.set(tab),
                }
                TabButton {
                    label: "管理",
                    tab: Tab::Settings,
                    current_tab: current_tab(),
                    onselect: move |tab| current_tab.set(tab),
                }
                TabButton {
                    label: "ヘルプ",
                    tab: Tab::Help,
                    current_tab: current_tab(),
                    onselect: move |tab| current_tab.set(tab),
                }
            }
            main { class: "workspace",
                EditTab { active: current_tab() == Tab::Edit }
                ExportTab { active: current_tab() == Tab::Export }
                SettingsTab { active: current_tab() == Tab::Settings }
                HelpTab { active: current_tab() == Tab::Help }
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
