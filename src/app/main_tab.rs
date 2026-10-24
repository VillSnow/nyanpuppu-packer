#![allow(non_snake_case)]

mod delete_confirmation;
mod editing_emoji;
mod emojis_grid;
mod import_emojis;
mod matching;

use std::collections::HashSet;

use crate::backend::{EmojiMatch, ExportEmoji, export_emoji_package};
use delete_confirmation::DeleteConfirmation;
use dioxus::{html::u::min_width, prelude::*};
use editing_emoji::{EditingEmoji, EditingEmojis};
use emojis_grid::{ColumnVisibilities, EmojisGrid, use_column_visibilities};
use import_emojis::import_images;
use matching::{MatchingOverlay, MatchingState, rematch_emojis, use_matching_state};

/// 「パッケージ作成」タブ。画像の読み込み・照合・編集・書き出しと、それに伴う状態を保持する。
#[component]
pub fn MainTab(active: bool) -> Element {
    let emojis = use_context_provider(|| EditingEmojis::new(Vec::new()));
    let match_threshold = use_signal(|| 10u32);
    let matching = use_matching_state();
    let export_message = use_signal(String::new);
    let pending_delete_count = use_signal(|| None::<usize>);
    let visibilities = use_column_visibilities();

    let current_emojis = emojis.read().clone();
    let current_matching_progress = (matching.progress)();

    rsx! {
        div { class: "workspace-content main-tab", hidden: !active,
            ImageAddToolbar { match_threshold, matching }
            if current_emojis.is_empty() {
                section {
                    p { "画像を追加すると、照合結果がここに表示されます。" }
                }
            } else {
                section { class: "emojis-section",
                    EmojisToolbar {
                        export_message,
                        pending_delete_count,
                        visibilities,
                    }
                    EmojisGrid { visibilities }
                }
            }
        }
        if matching.is_active() {
            MatchingOverlay { progress: current_matching_progress }
        }
        if let Some(count) = pending_delete_count() {
            DeleteConfirmation { count, pending_delete_count }
        }
    }
}

/// 一括操作の対象となるフィールド。
#[derive(Clone, Copy, PartialEq)]
enum BulkField {
    Category,
    EmojiName,
    Aliases,
    License,
}

impl BulkField {
    const ALL: [BulkField; 4] = [
        BulkField::Category,
        BulkField::EmojiName,
        BulkField::Aliases,
        BulkField::License,
    ];

    fn key(self) -> &'static str {
        match self {
            BulkField::Category => "category",
            BulkField::EmojiName => "emoji_name",
            BulkField::Aliases => "aliases",
            BulkField::License => "license",
        }
    }

    fn label(self) -> &'static str {
        match self {
            BulkField::Category => "カテゴリ",
            BulkField::EmojiName => "絵文字名",
            BulkField::Aliases => "タグ",
            BulkField::License => "ライセンス",
        }
    }

    fn from_key(key: &str) -> Option<BulkField> {
        Self::ALL.into_iter().find(|field| field.key() == key)
    }

    /// 固定値を適用する。タグは置き換えず、未登録の場合のみ追加する。
    fn apply_fixed(self, emoji: &mut EditingEmoji, value: &str) {
        match self {
            BulkField::Category => emoji.category = value.to_owned(),
            BulkField::EmojiName => emoji.emoji_name = value.to_owned(),
            BulkField::Aliases => add_alias(emoji, value),
            BulkField::License => emoji.license = value.to_owned(),
        }
    }

    fn clear(self, emoji: &mut EditingEmoji) {
        match self {
            BulkField::Category => emoji.category.clear(),
            BulkField::EmojiName => emoji.emoji_name.clear(),
            BulkField::Aliases => emoji.aliases.clear(),
            BulkField::License => emoji.license.clear(),
        }
    }

    fn apply_candidate(self, emoji: &mut EditingEmoji, candidate: &EmojiMatch) {
        match self {
            BulkField::Category => {
                emoji.category = candidate.category.as_deref().unwrap_or("").trim().to_owned();
            }
            BulkField::EmojiName => emoji.emoji_name = candidate.name.trim().to_owned(),
            BulkField::Aliases => {
                for tag in &candidate.aliases {
                    add_alias(emoji, tag);
                }
            }
            BulkField::License => {
                emoji.license = candidate.license.as_deref().unwrap_or("").trim().to_owned();
            }
        }
    }
}

fn add_alias(emoji: &mut EditingEmoji, tag: &str) {
    let tag = tag.trim();
    if !tag.is_empty() && !emoji.aliases.iter().any(|existing| existing == tag) {
        emoji.aliases.push(tag.to_owned());
    }
}

#[component]
fn LabeledCheckbox(label: String, mut flag: Signal<bool>) -> Element {
    rsx! {
        label {
            input {
                r#type: "checkbox",
                checked: flag(),
                onchange: move |event| flag.set(event.checked()),
            }
            "{label}"
        }
    }
}

#[component]
fn ImageAddToolbar(match_threshold: Signal<u32>, matching: MatchingState) -> Element {
    let emojis: EditingEmojis = use_context();
    let mut threshold_change_id = use_signal(|| 0u64);

    let matched_emoji_count = emojis
        .read()
        .iter()
        .filter(|emoji| !emoji.read().matches.is_empty())
        .count();
    let emoji_count = emojis.read().len();

    rsx! {
        section { class: "image-add-toolbar",
            div { class: "image-toolbar-primary",
                label { class: "image-file-picker",
                    "画像を追加"
                    input {
                        id: "image-input",
                        r#type: "file",
                        accept: "image/*",
                        multiple: true,
                        onchange: move |event| import_images(event.files(), emojis, match_threshold, matching),
                    }
                }
            }
            div { class: "image-toolbar-settings",
                label { r#for: "match-threshold",
                    "照合閾値"
                    span { class: "match-threshold-value", "{match_threshold}" }
                }
                input {
                    id: "match-threshold",
                    r#type: "range",
                    min: "0",
                    max: "20",
                    step: "1",
                    value: "{match_threshold}",
                    aria_label: "画像照合の閾値",
                    oninput: move |event| {
                        if let Ok(value) = event.value().parse::<u32>() {
                            match_threshold.set(value);
                            threshold_change_id.set(threshold_change_id() + 1);
                            let current_change_id = threshold_change_id();
                            spawn(async move {
                                gloo_timers::future::TimeoutFuture::new(2000).await;
                                if threshold_change_id() == current_change_id {
                                    rematch_emojis(emojis, match_threshold, value, matching);
                                }
                            });
                        }
                    },
                }
            }
            p { class: "emoji-match-status",
                "一致あり: {matched_emoji_count} / {emoji_count} 枚"
            }
        }
    }
}

#[component]
fn EmojisToolbar(
    export_message: Signal<String>,
    pending_delete_count: Signal<Option<usize>>,
    visibilities: ColumnVisibilities,
) -> Element {
    rsx! {
        div { class: "emojis-toolbars",
            SelectionActionsCard { export_message, pending_delete_count }
            ColumnVisibilityCard { visibilities }
        }
    }
}

#[component]
fn SelectionActionsCard(
    mut export_message: Signal<String>,
    mut pending_delete_count: Signal<Option<usize>>,
) -> Element {
    let emojis: EditingEmojis = use_context();
    let selected_count = emojis
        .read()
        .iter()
        .filter(|emoji| emoji.read().selected)
        .count();
    let mut exporting = use_signal(|| false);
    let mut target_field = use_signal(|| BulkField::Category);
    let mut fixed_value = use_signal(String::new);
    let mut chosen_source = use_signal(String::new);

    let sources = {
        let mut sources = HashSet::new();
        for emoji in emojis.read().iter() {
            for m in &emoji.read().matches {
                sources.insert(m.host.clone());
            }
        }
        let mut sources = sources.into_iter().collect::<Vec<_>>();
        sources.sort();
        sources
    };

    // 選択中の候補サーバーが存在しなくなった場合は先頭のサーバーに戻す
    let source = if sources.contains(&chosen_source()) {
        chosen_source()
    } else {
        sources.first().cloned().unwrap_or_default()
    };

    let wires_view_box_width = 300.0f64;

    let wire = |arg: i32, command: i32| -> Element {
        let gap = 50.0;
        let y1 = 50 + arg * 100;
        let y2 = 50 + command * 100;
        let theta = f64::atan2((y2 - y1) as f64, wires_view_box_width as f64);

        let x1 = gap * theta.cos();
        let x2 = wires_view_box_width - gap * theta.cos();
        let y1 = y1 as f64 + gap * theta.sin();
        let y2 = y2 as f64 - gap * theta.sin();

        rsx! {
            line {
                x1: "{x1}",
                y1: "{y1}",
                x2: "{x2}",
                y2: "{y2}",
                stroke: "black",
                marker_end: "url(#arrow)",
                vector_effect: "non-scaling-stroke",
            }
        }
    };

    rsx! {
        div { class: "emojis-toolbar",
            p { class: "emojis-toolbar-count", "操作対象: {selected_count} 件" }

            div { class: "emojis-toolbar-controls",
                div { class: "matrix-control",
                    div { class: "args",
                        label {
                            span { "対象フィールド" }
                            select {
                                value: target_field().key(),
                                onchange: move |event| {
                                    if let Some(field) = BulkField::from_key(&event.value()) {
                                        target_field.set(field);
                                    }
                                },
                                for field in BulkField::ALL {
                                    option { value: field.key(), "{field.label()}" }
                                }
                            }
                        }
                        label {
                            span { "固定値" }
                            input {
                                r#type: "text",
                                value: "{fixed_value}",
                                oninput: move |event| fixed_value.set(event.value()),
                            }
                        }
                        label {
                            span { "候補サーバー" }
                            select {
                                value: "{source}",
                                onchange: move |event| chosen_source.set(event.value()),
                                for source in sources.iter() {
                                    option { value: "{source}", "{source}" }
                                }
                            }
                        }
                    }
                    div { class: "wires",
                        svg { view_box: "0 0 {wires_view_box_width} 300",
                            defs {
                                marker {
                                    id: "arrow",
                                    view_box: "0 0 10 10",
                                    ref_x: "5",
                                    ref_y: "5",
                                    marker_width: "6",
                                    marker_height: "6",
                                    orient: "auto-start-reverse",
                                    path {
                                        d: "M 0 0 L 10 5 L 0 10 z",
                                        fill: "black",
                                    }
                                }
                            }
                            {wire(0, 0)}
                            {wire(0, 1)}
                            {wire(0, 2)}
                            {wire(1, 0)}
                            {wire(2, 1)}
                        }
                    }
                    div { class: "commands",
                        button {
                            r#type: "button",
                            style: "grid-row: 1;",
                            disabled: selected_count == 0,
                            onclick: move |_| {
                                let field = target_field();
                                let value = fixed_value();
                                for emoji in emojis.read().iter() {
                                    let mut emoji = *emoji;
                                    if emoji.read().selected {
                                        field.apply_fixed(&mut emoji.write(), &value);
                                    }
                                }
                            },
                            "フィールドに固定値を一括適用する"
                        }
                        button {
                            r#type: "button",
                            style: "grid-row: 2;",
                            disabled: selected_count == 0 || source.is_empty(),
                            onclick: {
                                let source = source.clone();
                                move |_| {
                                    let field = target_field();
                                    for emoji in emojis.read().iter() {
                                        let mut emoji = *emoji;
                                        if !emoji.read().selected {
                                            continue;
                                        }
                                        let candidate = emoji
                                            .read()
                                            .matches
                                            .iter()
                                            .filter(|m| m.host == source)
                                            .min_by_key(|m| m.distance)
                                            .cloned();
                                        if let Some(candidate) = candidate {
                                            field.apply_candidate(&mut emoji.write(), &candidate);
                                        }
                                    }
                                }
                            },
                            "フィールドに候補サーバーからの値を一括適用する"
                        }
                        button {
                            class: "emojis-toolbar-danger-button",
                            style: "grid-row: 3;",
                            r#type: "button",
                            disabled: selected_count == 0,
                            onclick: move |_| {
                                let field = target_field();
                                for emoji in emojis.read().iter() {
                                    let mut emoji = *emoji;
                                    if emoji.read().selected {
                                        field.clear(&mut emoji.write());
                                    }
                                }
                            },
                            "フィールドをクリアする"
                        }
                    }
                }

                div { class: "emojis-toolbar-actions",
                    button {
                        class: "emojis-toolbar-export-button",
                        r#type: "button",
                        disabled: selected_count == 0 || exporting(),
                        onclick: move |_| {
                            let export_emojis = emojis
                                .read()
                                .iter()
                                .filter(|emoji| emoji.read().selected)
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
                    button {
                        class: "emojis-toolbar-danger-button",
                        r#type: "button",
                        disabled: selected_count == 0,
                        onclick: move |_| pending_delete_count.set(Some(selected_count)),
                        "除去"
                    }
                }
            }
            if !export_message.read().is_empty() {
                p { style: "white-space: pre-line;", "{export_message}" }
            }
        }
    }
}

#[component]
fn ColumnVisibilityCard(visibilities: ColumnVisibilities) -> Element {
    rsx! {
        div { class: "emojis-toolbar column-visibility-card",
            div { class: "column-visibility-controls",
                LabeledCheckbox {
                    label: "ファイル名を表示",
                    flag: visibilities.file_name,
                }
                LabeledCheckbox {
                    label: "カテゴリを表示",
                    flag: visibilities.category_fields,
                }
                LabeledCheckbox {
                    label: "絵文字名を表示",
                    flag: visibilities.emoji_name_fields,
                }
                LabeledCheckbox { label: "タグを表示", flag: visibilities.alias_fields }
                LabeledCheckbox {
                    label: "ライセンスを表示",
                    flag: visibilities.license_fields,
                }
                LabeledCheckbox { label: "候補を表示", flag: visibilities.candidates }
            }
        }
    }
}
