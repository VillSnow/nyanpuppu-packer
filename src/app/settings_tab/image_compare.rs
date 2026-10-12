#![allow(non_snake_case)]

use crate::backend::{ImageComparison, compare_images};
use dioxus::prelude::*;

#[derive(Clone, PartialEq)]
struct ComparisonInput {
    bytes: Option<Vec<u8>>,
    file_name: String,
}

#[derive(Clone, Copy, PartialEq)]
enum ComparisonSource {
    File,
    Url,
}

impl ComparisonInput {
    fn new() -> Self {
        Self {
            bytes: None,
            file_name: String::new(),
        }
    }
}

#[component]
pub fn ImageCompare() -> Element {
    let left = use_signal(ComparisonInput::new);
    let right = use_signal(ComparisonInput::new);
    let left_url = use_signal(String::new);
    let right_url = use_signal(String::new);
    let left_source = use_signal(|| ComparisonSource::File);
    let right_source = use_signal(|| ComparisonSource::File);
    let mut result = use_signal(|| None::<ImageComparison>);
    let mut error = use_signal(String::new);
    let mut comparing = use_signal(|| false);

    rsx! {
        section { class: "image-compare",
            h2 { "画像比較" }
            p {
                "ローカルファイルまたは URL を2つ指定すると、グリッド化画像とハッシュの距離を比較します。"
            }
            form {
                class: "image-compare-form",
                onsubmit: move |event| {
                    event.prevent_default();
                    let left_input = left();
                    let right_input = right();
                    let left_url_value = left_url().trim().to_string();
                    let right_url_value = right_url().trim().to_string();
                    let left_image = (left_source() == ComparisonSource::File)
                        .then_some(left_input.bytes)
                        .flatten();
                    let right_image = (right_source() == ComparisonSource::File)
                        .then_some(right_input.bytes)
                        .flatten();
                    let left_url = (left_source() == ComparisonSource::Url)
                        .then_some(left_url_value)
                        .unwrap_or_default();
                    let right_url = (right_source() == ComparisonSource::Url)
                        .then_some(right_url_value)
                        .unwrap_or_default();
                    if left_image.is_none() && left_url.is_empty()
                        || right_image.is_none() && right_url.is_empty()
                    {
                        error.set("左右両方の画像を指定してください".to_string());
                        result.set(None);
                        return;
                    }
                    comparing.set(true);
                    error.set(String::new());
                    spawn(async move {
                        match compare_images(left_image, left_url, right_image, right_url).await {
                            Ok(comparison) => result.set(Some(comparison)),
                            Err(message) => {
                                result.set(None);
                                error.set(message);
                            }
                        }
                        comparing.set(false);
                    });
                },
                div { class: "image-compare-inputs",
                    ImageCompareInput {
                        title: "左の画像",
                        source_name: "left-source",
                        input: left,
                        url: left_url,
                        source: left_source,
                        error,
                    }
                    ImageCompareInput {
                        title: "右の画像",
                        source_name: "right-source",
                        input: right,
                        url: right_url,
                        source: right_source,
                        error,
                    }
                }
                button { r#type: "submit", disabled: comparing(),
                    if comparing() {
                        "比較中…"
                    } else {
                        "比較する"
                    }
                }
            }
            if !error.read().is_empty() {
                p { class: "image-compare-error", role: "alert", "{error}" }
            }
            if let Some(comparison) = result() {
                div { class: "image-compare-results",
                    h3 { class: "image-compare-result-title", "左の画像" }
                    h3 { class: "image-compare-result-title", "右の画像" }
                    figure { class: "image-compare-original",
                        img {
                            src: "{comparison.left.original}",
                            alt: "左の画像の元画像",
                        }
                        figcaption { "元画像" }
                    }
                    figure { class: "image-compare-original",
                        img {
                            src: "{comparison.right.original}",
                            alt: "右の画像の元画像",
                        }
                        figcaption { "元画像" }
                    }
                    ComparisonFrames {
                        title: "左の画像",
                        frames: comparison.left.frames,
                        class: "image-compare-frames",
                        frame_label: "グリッド",
                    }
                    ComparisonFrames {
                        title: "右の画像",
                        frames: comparison.right.frames,
                        class: "image-compare-frames",
                        frame_label: "グリッド",
                    }
                    p { class: "image-compare-hash-visualization-label", "ハッシュ可視化" }
                    p { class: "image-compare-hash-visualization-label", "ハッシュ可視化" }
                    ComparisonFrames {
                        title: "左の画像",
                        frames: comparison.left.hash_visualization,
                        class: "image-compare-frames image-compare-hash-visualization",
                        frame_label: "周波数面",
                    }
                    ComparisonFrames {
                        title: "右の画像",
                        frames: comparison.right.hash_visualization,
                        class: "image-compare-frames image-compare-hash-visualization",
                        frame_label: "周波数面",
                    }
                    p { class: "image-compare-hash",
                        "ハッシュ: "
                        code { "{comparison.left.hash}" }
                    }
                    p { class: "image-compare-hash",
                        "ハッシュ: "
                        code { "{comparison.right.hash}" }
                    }
                    if let Some(frames) = comparison.difference_frames {
                        section { class: "image-compare-difference",
                            h3 { "グリッド画像の差分" }
                            p { "赤: 左の画像が高輝度　緑: 右の画像が高輝度" }
                            ComparisonFrames {
                                title: "グリッド画像の差分",
                                frames,
                                class: "image-compare-frames",
                                frame_label: "グリッド",
                            }
                        }
                    }
                    p { class: "image-compare-distance",
                        "距離: "
                        strong { "{comparison.distance}" }
                    }
                }
            }
        }
    }
}

#[component]
fn ImageCompareInput(
    title: &'static str,
    source_name: &'static str,
    mut input: Signal<ComparisonInput>,
    mut url: Signal<String>,
    mut source: Signal<ComparisonSource>,
    mut error: Signal<String>,
) -> Element {
    rsx! {
        fieldset { class: "image-compare-input",
            legend { "{title}" }
            div { class: "image-compare-source-selector",
                label {
                    input {
                        r#type: "radio",
                        name: "{source_name}",
                        checked: source() == ComparisonSource::File,
                        onchange: move |_| source.set(ComparisonSource::File),
                    }
                    "ローカルファイル"
                }
                label {
                    input {
                        r#type: "radio",
                        name: "{source_name}",
                        checked: source() == ComparisonSource::Url,
                        onchange: move |_| source.set(ComparisonSource::Url),
                    }
                    "URL"
                }
            }
            label {
                "ローカルファイル"
                input {
                    r#type: "file",
                    accept: "image/*",
                    disabled: source() != ComparisonSource::File,
                    onchange: move |event| {
                        let files = event.files();
                        if let Some(file) = files.into_iter().next() {
                            spawn(async move {
                                match file.read_bytes().await {
                                    Ok(bytes) => {
                                        input
                                            .set(ComparisonInput {
                                                bytes: Some(bytes.to_vec()),
                                                file_name: file.name(),
                                            })
                                    }
                                    Err(message) => {
                                        error
                                            .set(
                                                format!(
                                                    "画像ファイルを読み込めません: {message}",
                                                ),
                                            );
                                        input.set(ComparisonInput::new());
                                    }
                                }
                            });
                        }
                    },
                }
            }
            if !input().file_name.is_empty() {
                p { class: "image-compare-file-name", "{input().file_name}" }
            }
            label {
                "URL"
                input {
                    r#type: "url",
                    placeholder: "https://example.com/image.png",
                    value: "{url}",
                    disabled: source() != ComparisonSource::Url,
                    oninput: move |event| url.set(event.value()),
                }
            }
        }
    }
}

#[component]
fn ComparisonFrames(
    title: &'static str,
    frames: Vec<String>,
    class: &'static str,
    frame_label: &'static str,
) -> Element {
    rsx! {
        div { class: "{class}",
            for (index , frame) in frames.iter().enumerate() {
                figure { key: "{index}",
                    img {
                        src: "data:image/png;base64,{frame}",
                        alt: "{title} グリッド {index + 1}",
                    }
                    figcaption { "{frame_label} {index + 1}" }
                }
            }
        }
    }
}
