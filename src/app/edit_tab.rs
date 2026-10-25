#![allow(non_snake_case)]

mod delete_confirmation;
mod editing_emoji;
mod emojis_grid;
mod import_emojis;
mod matching;
mod replace_confirmation;

use std::collections::BTreeMap;

use crate::backend::EmojiMatch;
use delete_confirmation::DeleteConfirmation;
use dioxus::{html::FileData, prelude::*};
use editing_emoji::EditingEmoji;
pub use editing_emoji::EditingEmojis;
use emojis_grid::{ColumnVisibilities, EmojisGrid, use_column_visibilities};
use import_emojis::{import_images, import_package};
use matching::{MatchingOverlay, MatchingState, rematch_emojis, use_matching_state};
use replace_confirmation::ReplaceConfirmation;

/// 「編集」タブ。画像の読み込み・照合・編集と、それに伴う状態を保持する。
/// 編集中の絵文字リストは書き出しタブと共有するため、コンテキストは App が提供する。
#[component]
pub fn EditTab(active: bool) -> Element {
    let emojis: EditingEmojis = use_context();
    let match_threshold = use_signal(|| 10u32);
    let matching = use_matching_state();
    let pending_delete_count = use_signal(|| None::<usize>);
    let pending_zip = use_signal(|| None::<FileData>);
    let import_message = use_signal(String::new);
    // EditTab のスコープで実行させるため、確認ダイアログではなくここでコールバックを作る
    let confirm_zip_import = use_callback(move |file: FileData| {
        import_package(file, emojis, match_threshold, matching, import_message);
    });
    let visibilities = use_column_visibilities();

    let current_emojis = emojis.read().clone();
    let current_matching_progress = (matching.progress)();

    rsx! {
        div { class: "workspace-content edit-tab", hidden: !active,
            ImageAddToolbar {
                match_threshold,
                matching,
                pending_zip,
                import_message,
            }
            if current_emojis.is_empty() {
                section {
                    p { "画像を追加すると、照合結果がここに表示されます。" }
                }
            } else {
                section { class: "emojis-section",
                    EmojisToolbar { pending_delete_count, visibilities }
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
        if pending_zip().is_some() {
            ReplaceConfirmation { pending_zip, on_confirm: confirm_zip_import }
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
                emoji.category = candidate
                    .category
                    .as_deref()
                    .unwrap_or("")
                    .trim()
                    .to_owned();
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
fn ImageAddToolbar(
    match_threshold: Signal<u32>,
    matching: MatchingState,
    mut pending_zip: Signal<Option<FileData>>,
    import_message: Signal<String>,
) -> Element {
    let emojis: EditingEmojis = use_context();
    let mut threshold_change_id = use_signal(|| 0u64);
    // 同じファイルを再度選択しても change が発火するよう、選択のたびに input を作り直す
    let mut zip_input_key = use_signal(|| 0u64);

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
                label { class: "image-file-picker",
                    "ZIPを読み込み"
                    input {
                        key: "{zip_input_key}",
                        id: "zip-input",
                        r#type: "file",
                        accept: ".zip,application/zip",
                        onchange: move |event| {
                            let Some(file) = event.files().into_iter().next() else {
                                return;
                            };
                            zip_input_key += 1;
                            if emojis.read().is_empty() {
                                import_package(file, emojis, match_threshold, matching, import_message);
                            } else {
                                pending_zip.set(Some(file));
                            }
                        },
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
            if !import_message.read().is_empty() {
                p {
                    class: "emoji-match-status",
                    style: "white-space: pre-line;",
                    "{import_message}"
                }
            }
        }
    }
}

#[component]
fn EmojisToolbar(
    pending_delete_count: Signal<Option<usize>>,
    visibilities: ColumnVisibilities,
) -> Element {
    rsx! {
        div { class: "emojis-toolbars",
            SelectionActionsCard { pending_delete_count }
            ColumnVisibilityCard { visibilities }
        }
    }
}

#[component]
fn SelectionActionsCard(mut pending_delete_count: Signal<Option<usize>>) -> Element {
    let emojis: EditingEmojis = use_context();
    let selected_count = emojis
        .read()
        .iter()
        .filter(|emoji| emoji.read().selected)
        .count();
    let mut target_field = use_signal(|| BulkField::Category);
    let mut fixed_value = use_signal(String::new);
    let mut chosen_source = use_signal(String::new);

    let sources = {
        let mut sources = BTreeMap::new();
        for emoji in emojis.read().iter() {
            for m in &emoji.read().matches {
                sources.insert((m.host_rank, m.host.clone()), m.host_label.clone());
            }
        }
        sources
            .into_iter()
            .map(|((_, host), label)| (host, label))
            .collect::<Vec<_>>()
    };

    // 選択中の候補サーバーが存在しなくなった場合は先頭のサーバーに戻す
    let source = if sources.iter().any(|(host, _)| *host == chosen_source()) {
        chosen_source()
    } else {
        sources
            .first()
            .map(|(host, _)| host.clone())
            .unwrap_or_default()
    };

    let wires_view_box_width = 300.0f64;

    let wire = |arg: i32, command: i32| -> Element {
        let gap = 0.0;
        let smooth = wires_view_box_width / 2.0;
        let y1 = 50 + arg * 100;
        let y2 = 50 + command * 100;
        let theta = f64::atan2((y2 - y1) as f64, wires_view_box_width as f64);

        let x1 = gap * theta.cos();
        let x2 = wires_view_box_width - gap * theta.cos();
        let y1 = y1 as f64 + gap * theta.sin();
        let y2 = y2 as f64 - gap * theta.sin();

        rsx! {
            path {
                d: "M {x1} {y1} C {x1+smooth} {y1}, {x2-smooth} {y2}, {x2} {y2}",
                stroke: "black",
                fill: "none",
                marker_end: "url(#arrow)",
                vector_effect: "non-scaling-stroke",
            }
            // line {
            //     x1: "{x1}",
            //     y1: "{y1}",
            //     x2: "{x2}",
            //     y2: "{y2}",
            //     stroke: "black",
            //     marker_end: "url(#arrow)",
            //     vector_effect: "non-scaling-stroke",
            // }
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
                            span { "検索値" }
                            input {
                                r#type: "text",
                                value: "{fixed_value}",
                                oninput: move |event| fixed_value.set(event.value()),
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
                                for (host , label) in sources.iter() {
                                    option { value: "{host}", "{label}" }
                                }
                            }
                        }
                    }
                    div { class: "wires",
                        svg { view_box: "0 0 {wires_view_box_width} 500",
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
                            {wire(2, 0)}
                            {wire(0, 1)}
                            {wire(1, 1)}
                            {wire(2, 1)}
                            {wire(0, 2)}
                            {wire(3, 2)}
                            {wire(0, 3)}
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
                            "フィールドの文字列を検索して固定値に置換する"
                        }
                        button {
                            r#type: "button",
                            style: "grid-row: 3;",
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
                            "フィールドに候補サーバーの値を一括適用する"
                        }
                        button {
                            class: "emojis-toolbar-danger-button",
                            style: "grid-row: 4;",
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
                        button {
                            class: "emojis-toolbar-danger-button",
                            style: "grid-row: 5;",
                            r#type: "button",
                            disabled: selected_count == 0,
                            onclick: move |_| pending_delete_count.set(Some(selected_count)),
                            "絵文字を除去する"
                        }
                    }
                }

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
