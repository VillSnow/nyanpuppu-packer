#![allow(non_snake_case)]

use super::editing_emoji::{EditingEmoji, EditingEmojis, EditingEmojisExt};
use crate::backend::EmojiMatch;
use dioxus::prelude::*;

use std::collections::HashMap;

/// グリッドの列・候補の表示切替。
#[derive(Clone, Copy, PartialEq)]
pub struct ColumnVisibilities {
    pub file_name: Signal<bool>,
    pub emoji_name_fields: Signal<bool>,
    pub license_fields: Signal<bool>,
    pub alias_fields: Signal<bool>,
    pub candidates: Signal<bool>,
}

pub fn use_column_visibilities() -> ColumnVisibilities {
    ColumnVisibilities {
        file_name: use_signal(|| true),
        emoji_name_fields: use_signal(|| true),
        license_fields: use_signal(|| true),
        alias_fields: use_signal(|| true),
        candidates: use_signal(|| true),
    }
}

fn group_matches_by_key<F, I>(matches: &[EmojiMatch], keys: F) -> Vec<(String, Vec<EmojiMatch>)>
where
    F: Fn(&EmojiMatch) -> I,
    I: IntoIterator<Item = String>,
{
    let mut groups = HashMap::new();

    for candidate in matches {
        let keys = keys(candidate);
        for key in keys {
            if key.is_empty() {
                continue;
            }

            groups
                .entry(key)
                .or_insert_with(Vec::new)
                .push(candidate.clone());
        }
    }

    let mut result = groups
        .into_iter()
        .collect::<Vec<(String, Vec<EmojiMatch>)>>();
    result.sort_by(|a, b| a.0.cmp(&b.0));

    result
}

// グリッドのトラック名
const CHK_AND_PREVIEW: &str = "chk_and_preview";
const FILE_NAME: &str = "file_name";
const EMOJI_NAME_FIELDS: &str = "emoji_name_fields";
const EMOJI_NAME_CANDIDATE: &str = "emoji_name_candidate";
const EMOJI_NAME_SOURCE: &str = "emoji_name_source";
const ALIAS_FIELDS: &str = "alias_fields";
const ALIAS_CANDIDATE: &str = "alias_candidate";
const ALIAS_SOURCE: &str = "alias_source";
const LICENSE_FIELDS: &str = "license_fields";
const LICENSE_CANDIDATE: &str = "license_candidate";
const LICENSE_SOURCE: &str = "license_source";

#[component]
pub fn EmojisGrid(emojis: Vec<EditingEmoji>, visibilities: ColumnVisibilities) -> Element {
    let grid_template_columns = {
        fn track(name: &str, value: &str, vis: bool) -> String {
            if vis {
                format!("[{name}] fit-content(50%)")
            } else {
                format!("[{name}] 0")
            }
        }
        [
            // チェックボックスとプレビュー画像
            track(CHK_AND_PREVIEW, "max-content", true),
            // ファイル名
            track(
                FILE_NAME,
                "minmax(max-content, auto)",
                (visibilities.file_name)(),
            ),
            // 絵文字名
            track(
                EMOJI_NAME_FIELDS,
                "minmax(max-content, auto)",
                (visibilities.emoji_name_fields)(),
            ),
            track(
                EMOJI_NAME_CANDIDATE,
                "minmax(max-content, auto)",
                (visibilities.emoji_name_fields)() && (visibilities.candidates)(),
            ),
            track(
                EMOJI_NAME_SOURCE,
                "minmax(max-content, auto)",
                (visibilities.emoji_name_fields)() && (visibilities.candidates)(),
            ),
            // タグ
            track(
                ALIAS_FIELDS,
                "minmax(max-content, auto)",
                (visibilities.alias_fields)(),
            ),
            track(
                ALIAS_CANDIDATE,
                "fit-content(50%)",
                (visibilities.alias_fields)() && (visibilities.candidates)(),
            ),
            track(
                ALIAS_SOURCE,
                "minmax(max-content, auto)",
                (visibilities.alias_fields)() && (visibilities.candidates)(),
            ),
            // ライセンス
            track(LICENSE_FIELDS, "auto", (visibilities.license_fields)()),
            track(
                LICENSE_CANDIDATE,
                "minmax(max-content, auto)",
                (visibilities.license_fields)() && (visibilities.candidates)(),
            ),
            track(
                LICENSE_SOURCE,
                "minmax(max-content, auto)",
                (visibilities.license_fields)() && (visibilities.candidates)(),
            ),
        ]
        .join(" ")
    };

    rsx! {

        div {
            class: "emojis-grid",
            style: "grid-template-columns: {grid_template_columns};",
            EmojiGridHeader { visibilities }
            for emoji in emojis {
                EmojiGridRows {
                    key: "{emoji.md5}",
                    md5: emoji.md5.clone(),
                    visibilities,
                }
            }
        }

    }
}

#[component]
fn EmojiGridHeader(visibilities: ColumnVisibilities) -> Element {
    let mut emojis: EditingEmojis = use_context();

    rsx! {
        div { class: "row header-row",
            div { class: "cell", style: "grid-column: {CHK_AND_PREVIEW}",
                button {
                    r#type: "button",
                    onclick: move |_| {
                        emojis.with_mut(|all| all.iter_mut().for_each(|emoji| emoji.selected = true))
                    },
                    "全選択"
                }
                button {
                    r#type: "button",
                    onclick: move |_| {
                        emojis.with_mut(|all| all.iter_mut().for_each(|emoji| emoji.selected = false))
                    },
                    "全解除"
                }
            }
            div {
                class: "cell",
                style: "grid-column: {FILE_NAME}",
                hidden: !(visibilities.file_name)(),
                "ファイル名"
            }
            div {
                class: "cell",
                hidden: !(visibilities.emoji_name_fields)(),
                style: "grid-column: {EMOJI_NAME_FIELDS} / span 3",
                "絵文字名"
            }
            div {
                class: "cell",
                hidden: !(visibilities.alias_fields)(),
                style: "grid-column: {ALIAS_FIELDS} / span 3",
                "タグ"
            }
            div {
                class: "cell",
                hidden: !(visibilities.license_fields)(),
                style: "grid-column: {LICENSE_FIELDS} / span 3",
                "ライセンス"
            }
        }
    }
}

#[component]
fn EmojiGridRows(md5: String, visibilities: ColumnVisibilities) -> Element {
    let emojis: EditingEmojis = use_context();
    let emoji = emojis
        .read()
        .iter()
        .find(|emoji| emoji.md5 == md5)
        .unwrap()
        .clone();

    let emoji_name_candidates = group_matches_by_key(&emoji.matches, |candidate| {
        [candidate.name.trim().to_owned()]
    });

    let alias_candidates = group_matches_by_key(&emoji.matches, |candidate| {
        candidate
            .aliases
            .iter()
            .map(|tag| tag.trim().to_owned())
            .collect::<Vec<String>>()
    });

    let license_candidates = group_matches_by_key(&emoji.matches, |candidate| {
        [candidate.license.as_deref().unwrap_or("").trim().to_owned()]
    });

    rsx! {
        div { class: "row",
            // チェックボックスとプレビュー画像
            div { class: "cell", style: "grid-column: {CHK_AND_PREVIEW}",
                div { class: "organelle sticky-item toggle-and-preview",
                    input {
                        id: "emoji-select-{md5}",
                        r#type: "checkbox",
                        checked: emoji.selected,
                        aria_label: "{emoji.file_name} を選択",
                        onchange: {
                            let mut emojis = emojis.clone();
                            let md5 = md5.clone();
                            move |event| {
                                emojis
                                    .with_mut(|all| {
                                        all
                                            .iter_mut()
                                            .for_each(|emoji| {
                                                if emoji.md5 == md5 {
                                                    emoji.selected = event.checked();
                                                }
                                            })
                                    })
                            }
                        },
                    }
                    label { class: "preview-toggle", r#for: "emoji-select-{md5}",
                        img { src: "{emoji.preview}", alt: "" }
                    }
                }
            }

            // ファイル名
            div {
                class: "cell",
                style: "grid-column: file_name",
                hidden: !(visibilities.file_name)(),
                div { class: "organelle sticky-item",
                    div { "{emoji.file_name}" }
                }
            }

            // 絵文字名入力
            div {
                class: "cell",
                style: "grid-column: emoji_name_fields",
                hidden: !(visibilities.emoji_name_fields)(),
                div { class: "organelle sticky-item",

                    input {
                        r#type: "text",
                        placeholder: "絵文字名",
                        value: "{emoji.emoji_name}",
                        oninput: {
                            let mut emojis = emojis.clone();
                            let md5 = md5.clone();
                            move |event| {
                                emojis
                                    .with_mut(|emojis| {
                                        let emoji = emojis.get_by_md5_mut(&md5).unwrap();
                                        emoji.emoji_name = event.value();
                                    })
                            }
                        },
                    }

                }
            }

            // 絵文字名候補
            {
                candidate_buttons(
                    emoji.md5.clone(),
                    EMOJI_NAME_CANDIDATE,
                    (visibilities.emoji_name_fields)() && (visibilities.candidates)(),
                    &emoji_name_candidates,
                    emojis.clone(),
                    |emoji, emoji_name| { emoji.emoji_name == emoji_name },
                    |emoji, emoji_name| {
                        emoji.emoji_name = emoji_name.clone();
                    },
                )
            }

            // 絵文字名候補のホスト
            {
                source_host_labels(
                    EMOJI_NAME_SOURCE,
                    (visibilities.emoji_name_fields)() && (visibilities.candidates)(),
                    &emoji_name_candidates,
                )
            }

            // エイリアス入力
            div {
                class: "cell",
                style: "grid-column: {ALIAS_FIELDS};",
                hidden: !(visibilities.alias_fields)(),
                div {
                    class: "organelle",
                    style: "min-height: calc(var(--organelle-unit) * 3); height: 100%;",
                    textarea {
                        placeholder: "タグを1行ずつ入力",
                        value: emoji.aliases.join("\n"),
                        oninput: {
                            let mut emojis = emojis.clone();
                            let md5 = md5.clone();
                            move |event| {
                                emojis
                                    .with_mut(|emojis| {
                                        let emoji = emojis.get_by_md5_mut(&md5).unwrap();
                                        let tags = event
                                            .value()
                                            .lines()
                                            .filter(|tag| !tag.is_empty())
                                            .map(str::to_string)
                                            .collect();
                                        emoji.aliases = tags;
                                    });
                            }
                        },
                    }
                }
            }

            // エイリアス候補
            {
                candidate_buttons(
                    emoji.md5.clone(),
                    ALIAS_CANDIDATE,
                    (visibilities.alias_fields)() && (visibilities.candidates)(),
                    &alias_candidates,
                    emojis.clone(),
                    |emoji, tag| { emoji.aliases.contains(&tag) },
                    |emoji, tag| {
                        if emoji.aliases.contains(&tag) {
                            emoji.aliases.retain(|existing_tag| existing_tag != &tag);
                        } else {
                            emoji.aliases.push(tag.clone());
                        }
                    },
                )
            }

            // エイリアスのホスト
            {
                source_host_labels(
                    ALIAS_SOURCE,
                    (visibilities.alias_fields)() && (visibilities.candidates)(),
                    &alias_candidates,
                )
            }

            // ライセンス入力
            div {
                class: "cell",
                style: "grid-column: {LICENSE_FIELDS};",
                hidden: !(visibilities.license_fields)(),
                div { class: "organelle sticky-item",
                    input {
                        r#type: "text",
                        placeholder: "ライセンス",
                        value: "{emoji.license}",
                        oninput: {
                            let mut emojis = emojis.clone();
                            let md5 = md5.clone();
                            move |event| {
                                emojis
                                    .with_mut(|emojis| {
                                        let emoji = emojis.get_by_md5_mut(&md5).unwrap();
                                        emoji.license = event.value();
                                    })
                            }
                        },
                    }

                }
            }

            // ライセンス候補
            {
                candidate_buttons(
                    emoji.md5.clone(),
                    LICENSE_CANDIDATE,
                    (visibilities.license_fields)() && (visibilities.candidates)(),
                    &license_candidates,
                    emojis.clone(),
                    |emoji, license| { emoji.license == license },
                    |emoji, license| {
                        if emoji.license == license {
                            emoji.license.clear();
                        } else {
                            emoji.license = license.clone();
                        }
                    },
                )
            }

            // ライセンス候補のホスト
            {
                source_host_labels(
                    LICENSE_SOURCE,
                    (visibilities.license_fields)() && (visibilities.candidates)(),
                    &license_candidates,
                )
            }
        }
    }
}

fn candidate_buttons(
    emoji_md5_hash: String,
    column: &str,
    vis: bool,
    groups: &Vec<(String, Vec<EmojiMatch>)>,
    emojis: EditingEmojis,
    selected: fn(&EditingEmoji, String) -> bool,
    modify: fn(&mut EditingEmoji, String),
) -> Element {
    rsx! {
        div { class: "cell", style: "grid-column: {column}", hidden: !vis,
            div { class: "v-stack-start sticky-item",
                for (key , ms) in groups {
                    div {
                        class: "candidate-group",
                        style: "height: calc(var(--organelle-unit) * {ms.len()});",
                        div {
                            class: "organelle sticky-item",
                            class: if selected(emojis.read().get_by_md5(&emoji_md5_hash).unwrap(), key.clone()) { "selected" },
                            button {
                                onclick: {
                                    let mut emojis = emojis.clone();
                                    let emoji_md5_hash = emoji_md5_hash.clone();
                                    let key = key.clone();
                                    move |_| {
                                        emojis
                                            .with_mut(|emojis| {
                                                let emoji = emojis.get_by_md5_mut(&emoji_md5_hash).unwrap();
                                                modify(emoji, key.clone());
                                            })
                                    }
                                },
                                "{key}"
                            }
                        }
                    }
                }
            }
        }
    }
}

fn source_host_labels(column: &str, vis: bool, groups: &Vec<(String, Vec<EmojiMatch>)>) -> Element {
    rsx! {
        div { class: "cell", style: "grid-column: {column}", hidden: !vis,
            div { class: "v-stack-start sticky-item",
                for (_ , ms) in groups {
                    div { class: "candidate-group",
                        for m in ms {
                            div { class: "organelle",
                                span { "{m.host}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
