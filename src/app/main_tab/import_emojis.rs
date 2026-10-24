use super::{
    editing_emoji::{EditingEmoji, EditingEmojis},
    matching::{MatchingProgress, MatchingState},
};
use crate::backend::{calculate_image_hash, import_emoji_package, match_hash};
use base64::Engine;
use dioxus::prelude::*;
use futures_util::future::join_all;
use md5::{Digest, Md5};
use std::{collections::HashSet, rc::Rc};

/// 選択された画像ファイルを読み込み、ハッシュ計算と照合を行って編集中の絵文字として追加する。
pub fn import_images(
    files: Vec<dioxus::html::FileData>,
    mut emojis: EditingEmojis,
    threshold: Signal<u32>,
    matching: MatchingState,
) {
    let mut matching_progress = matching.progress;
    matching.begin();
    spawn(async move {
        let mut loaded = Vec::new();
        matching_progress.set(MatchingProgress {
            phase: "画像を読み込んでいます".to_string(),
            completed: 0,
            total: files.len() * 3,
        });
        gloo_timers::future::TimeoutFuture::new(0).await;

        for file in files {
            let name = file.name();
            let mime = file
                .content_type()
                .unwrap_or_else(|| "image/png".to_string());
            if let Ok(bytes) = file.read_bytes().await {
                let bytes = bytes.to_vec();
                let mut md5_digest = Md5::new();
                md5_digest.update(&bytes);
                let md5_hash = format!("{:x}", md5_digest.finalize());
                let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
                loaded.push(EditingEmoji {
                    file_name: name,
                    preview: format!("data:{mime};base64,{encoded}"),
                    bytes: Rc::new(bytes),
                    md5: md5_hash,
                    hash: None,
                    matches: Vec::new(),
                    error: None,
                    emoji_name: String::new(),
                    category: String::new(),
                    aliases: Vec::new(),
                    license: String::new(),
                    selected: true,
                });
            }
            matching_progress.with_mut(|progress| progress.completed += 1);
            gloo_timers::future::TimeoutFuture::new(0).await;
        }

        let mut existing_md5_hashes = HashSet::new();
        emojis.with(|current| {
            existing_md5_hashes.extend(current.iter().map(|emoji| emoji.read().md5.clone()));
        });
        loaded.retain(|emoji| existing_md5_hashes.insert(emoji.md5.clone()));

        hash_and_match(&mut loaded, threshold, matching_progress).await;

        let scope = emojis.origin_scope();
        emojis.with_mut(|current| {
            // 照合中にemojis が変更されている可能性があるので、既知のMD5ハッシュを収集して重複を排除する
            let mut known_md5_hashes = current
                .iter()
                .map(|emoji| emoji.read().md5.clone())
                .collect::<HashSet<_>>();
            loaded.retain(|emoji| known_md5_hashes.insert(emoji.md5.clone()));
            current.extend(
                loaded
                    .into_iter()
                    .map(|emoji| Signal::new_in_scope(emoji, scope)),
            );
        });
        matching.finish();
    });
}

/// 画像ハッシュの作成と候補の照合を行い、結果を `loaded` に反映する。
async fn hash_and_match(
    loaded: &mut Vec<EditingEmoji>,
    threshold: Signal<u32>,
    mut matching_progress: Signal<MatchingProgress>,
) {
    // Image hashes are independent, so calculate them concurrently.
    matching_progress.set(MatchingProgress {
        phase: "画像ハッシュを作成しています".to_string(),
        completed: matching_progress().completed,
        total: matching_progress().completed + loaded.len() * 2,
    });
    let hash_results = join_all(
        loaded
            .iter()
            .map(|emoji| calculate_image_hash(emoji.bytes.as_ref().clone())),
    )
    .await;
    for (emoji, result) in loaded.iter_mut().zip(hash_results) {
        match result {
            Ok(hash) => emoji.hash = Some(hash),
            Err(error) => emoji.error = Some(error),
        }

        matching_progress.with_mut(|progress| progress.completed += 1);
        gloo_timers::future::TimeoutFuture::new(0).await;
    }

    // Keep database matching serial, repeating the batch if its threshold
    // changed while the initial hashes were being calculated.
    let mut current_threshold = threshold();
    loop {
        matching_progress.with_mut(|progress| {
            progress.phase = "候補を照合しています".to_string();
        });
        for emoji in loaded.iter_mut() {
            let Some(hash) = emoji.hash.clone() else {
                matching_progress.with_mut(|progress| progress.completed += 1);
                gloo_timers::future::TimeoutFuture::new(0).await;
                continue;
            };
            match match_hash(hash, current_threshold).await {
                Ok(matches) => {
                    emoji.matches = matches;
                    emoji.error = None;
                }
                Err(error) => {
                    emoji.matches.clear();
                    emoji.error = Some(error);
                }
            }
            matching_progress.with_mut(|progress| progress.completed += 1);
            gloo_timers::future::TimeoutFuture::new(0).await;
        }
        let latest_threshold = threshold();
        if latest_threshold == current_threshold {
            break;
        }
        current_threshold = latest_threshold;
        matching_progress.with_mut(|progress| progress.total += loaded.len());
    }
}

/// 絵文字パッケージ (ZIP) を読み込み、編集中の絵文字をすべて置き換える。
/// 絵文字名・カテゴリ・タグ・ライセンスは ZIP の内容を引き継ぐ。
/// 読み込めなかった項目があれば `message` に理由を設定する。
pub fn import_package(
    file: dioxus::html::FileData,
    mut emojis: EditingEmojis,
    threshold: Signal<u32>,
    matching: MatchingState,
    mut message: Signal<String>,
) {
    let mut matching_progress = matching.progress;
    matching.begin();
    message.set(String::new());
    spawn(async move {
        matching_progress.set(MatchingProgress {
            phase: "ZIP を読み込んでいます".to_string(),
            completed: 0,
            total: 0,
        });
        gloo_timers::future::TimeoutFuture::new(0).await;

        let archive = match file.read_bytes().await {
            Ok(bytes) => bytes.to_vec(),
            Err(error) => {
                message.set(format!("ZIP を読み込めませんでした: {error}"));
                matching.finish();
                return;
            }
        };
        let package = match import_emoji_package(archive).await {
            Ok(package) => package,
            Err(error) => {
                message.set(error);
                matching.finish();
                return;
            }
        };

        let mut warnings = package.warnings;
        let mut known_md5_hashes = HashSet::new();
        let mut loaded = Vec::new();
        for emoji in package.emojis {
            let mut md5_digest = Md5::new();
            md5_digest.update(&emoji.image);
            let md5_hash = format!("{:x}", md5_digest.finalize());
            if !known_md5_hashes.insert(md5_hash.clone()) {
                warnings.push(format!(
                    "{}: 同じ画像が既に読み込まれているためスキップしました",
                    emoji.emoji_name
                ));
                continue;
            }
            let encoded = base64::engine::general_purpose::STANDARD.encode(&emoji.image);
            loaded.push(EditingEmoji {
                file_name: emoji.file_name,
                preview: format!("data:{};base64,{encoded}", emoji.mime),
                bytes: Rc::new(emoji.image),
                md5: md5_hash,
                hash: None,
                matches: Vec::new(),
                error: None,
                emoji_name: emoji.emoji_name,
                category: emoji.category,
                aliases: emoji.aliases,
                license: emoji.license,
                selected: true,
            });
        }

        hash_and_match(&mut loaded, threshold, matching_progress).await;

        let scope = emojis.origin_scope();
        emojis.with_mut(|current| {
            *current = loaded
                .into_iter()
                .map(|emoji| Signal::new_in_scope(emoji, scope))
                .collect();
        });
        if !warnings.is_empty() {
            message.set(format!(
                "読み込めなかった項目があります:\n{}",
                warnings.join("\n")
            ));
        }
        matching.finish();
    });
}
