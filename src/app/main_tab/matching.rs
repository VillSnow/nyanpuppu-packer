use super::editing_emoji::EditingEmojis;
use crate::backend::match_hash;
use dioxus::prelude::*;

#[derive(Clone, Default, PartialEq)]
pub struct MatchingProgress {
    pub phase: String,
    pub completed: usize,
    pub total: usize,
}

impl MatchingProgress {
    pub fn percent(&self) -> usize {
        if self.total == 0 {
            0
        } else {
            self.completed * 100 / self.total
        }
    }
}

/// 照合処理の実行状況。並行する処理の数を数え、1 件以上あればオーバーレイを表示する。
#[derive(Clone, Copy, PartialEq)]
pub struct MatchingState {
    count: Signal<usize>,
    pub progress: Signal<MatchingProgress>,
}

pub fn use_matching_state() -> MatchingState {
    MatchingState {
        count: use_signal(|| 0),
        progress: use_signal(MatchingProgress::default),
    }
}

impl MatchingState {
    pub fn is_active(&self) -> bool {
        (self.count)() > 0
    }

    pub fn begin(mut self) {
        self.count.with_mut(|count| *count += 1);
    }

    pub fn finish(mut self) {
        self.count
            .with_mut(|count| *count = count.saturating_sub(1));
    }
}

#[component]
pub fn MatchingOverlay(progress: MatchingProgress) -> Element {
    rsx! {
        div {
            class: "matching-overlay",
            role: "dialog",
            aria_modal: "true",
            aria_label: "画像を照合中",
            tabindex: "-1",
            div { class: "matching-dialog",
                div { class: "matching-spinner", aria_hidden: "true" }
                p { "画像を照合しています" }
                p { class: "matching-dialog-detail", "{progress.phase}" }
                progress {
                    class: "matching-progress",
                    max: "100",
                    value: "{progress.percent()}",
                }
                p { class: "matching-progress-detail",
                    "{progress.completed} / {progress.total} 件完了"
                }
            }
        }
    }
}

/// 閾値の変更後に、読み込み済みの全絵文字の候補を再照合する。
/// 処理中に `active_threshold` が変わった場合は中断する。
pub fn rematch_emojis(
    mut emojis: EditingEmojis,
    active_threshold: Signal<u32>,
    threshold: u32,
    matching: MatchingState,
) {
    let mut matching_progress = matching.progress;
    matching.begin();
    spawn(async move {
        matching_progress.set(MatchingProgress {
            phase: "候補を再照合しています".to_string(),
            completed: 0,
            total: 0,
        });
        gloo_timers::future::TimeoutFuture::new(0).await;
        let snapshots = emojis
            .read()
            .iter()
            .filter_map(|emoji| {
                emoji
                    .read()
                    .hash
                    .clone()
                    .map(|hash| (emoji.read().md5.clone(), hash))
            })
            .collect::<Vec<_>>();
        matching_progress.with_mut(|progress| progress.total = snapshots.len());
        for (md5, hash) in snapshots {
            let (matches, error) = match match_hash(hash, threshold).await {
                Ok(matches) => (matches, None),
                Err(error) => (Vec::new(), Some(error)),
            };
            if active_threshold() != threshold {
                break;
            }
            emojis.with_mut(|current| {
                if let Some(emoji) = current.iter_mut().find(|emoji| emoji.read().md5 == md5) {
                    emoji.with_mut(|emoji| {
                        emoji.matches = matches;
                        emoji.error = error;
                    });
                }
            });
            matching_progress.with_mut(|progress| progress.completed += 1);
            gloo_timers::future::TimeoutFuture::new(0).await;
        }
        matching.finish();
    });
}
