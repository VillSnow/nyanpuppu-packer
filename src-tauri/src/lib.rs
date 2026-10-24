mod database;
mod export;
mod image_hash;
mod import;

use database::{add_server, list_servers, remove_server};
use futures_util::{StreamExt, stream};
use serde::{Deserialize, Serialize};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tauri::Emitter;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[derive(Serialize)]
struct ExportSummary {
    path: String,
    emoji_count: usize,
}

/// 入力を検証し、保存先を選択させて絵文字パッケージ (ZIP) を書き出す。
/// 保存先の選択がキャンセルされた場合は `None` を返す。
#[tauri::command]
async fn export_emoji_package(
    app: tauri::AppHandle,
    emojis: Vec<export::ExportEmoji>,
) -> Result<Option<ExportSummary>, String> {
    let prepared = export::prepare(emojis).map_err(|e| e.to_string())?;

    let destination = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .add_filter("ZIPファイル", &["zip"])
            .set_file_name("emojis.zip")
            .blocking_save_file()
    })
    .await
    .map_err(|e| e.to_string())?;
    let Some(destination) = destination else {
        return Ok(None);
    };
    let path = destination.into_path().map_err(|e| e.to_string())?;

    let emoji_count = prepared.len();
    let written_path = path.clone();
    tauri::async_runtime::spawn_blocking(move || -> anyhow::Result<()> {
        let archive = export::build_package(&prepared)?;
        std::fs::write(&written_path, archive)?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| {
        tracing::error!(error = %e, "絵文字パッケージの書き出しに失敗しました");
        format!("ZIP を書き出せませんでした: {e}")
    })?;

    tracing::info!(path = %path.display(), emoji_count, "絵文字パッケージを書き出しました");
    Ok(Some(ExportSummary {
        path: path.display().to_string(),
        emoji_count,
    }))
}

/// 絵文字パッケージ (ZIP) の内容を読み込む。
#[tauri::command]
async fn import_emoji_package(archive: Vec<u8>) -> Result<import::ImportedPackage, String> {
    tauri::async_runtime::spawn_blocking(move || import::read_package(archive))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| {
            tracing::error!(error = %e, "絵文字パッケージの読み込みに失敗しました");
            format!("ZIP を読み込めませんでした: {e:#}")
        })
}

#[derive(Serialize)]
struct FetchSummary {
    emoji_count: usize,
    image_count: usize,
}

#[derive(Clone, Serialize)]
struct FetchProgress {
    server_id: i64,
    stage: String,
    completed: usize,
    total: usize,
}

#[derive(Deserialize)]
struct ComparisonSource {
    image: Option<Vec<u8>>,
    url: Option<String>,
}

#[derive(Serialize)]
struct ComparisonImage {
    original: String,
    frames: Vec<String>,
    hash: String,
    hash_visualization: Vec<String>,
}

#[derive(Serialize)]
struct ImageComparison {
    left: ComparisonImage,
    right: ComparisonImage,
    difference_frames: Option<Vec<String>>,
    distance: u32,
}

fn report_fetch_progress(
    app: &tauri::AppHandle,
    server_id: i64,
    stage: &str,
    completed: usize,
    total: usize,
) {
    if let Err(error) = app.emit(
        "emoji-fetch-progress",
        FetchProgress {
            server_id,
            stage: stage.to_string(),
            completed,
            total,
        },
    ) {
        tracing::warn!(server_id, error = %error, "進捗イベントをUIへ送信できませんでした");
    }
}

async fn fetch_image_with_backoff(
    client: &reqwest::Client,
    image_url: &str,
) -> anyhow::Result<DownloadedImage> {
    const MAX_ATTEMPTS: u8 = 4;
    const MAX_IMAGE_SIZE: u64 = 16 * 1024 * 1024;
    let mut delay = std::time::Duration::from_millis(250);

    for attempt in 0..MAX_ATTEMPTS {
        let (result, should_retry) = match client
            .get(image_url)
            .timeout(std::time::Duration::from_secs(20))
            .send()
            .await
        {
            Err(error) => (Err(error.into()), true),
            Ok(response) => {
                let status = response.status();
                if status.is_success() {
                    if response
                        .content_length()
                        .is_some_and(|size| size > MAX_IMAGE_SIZE)
                    {
                        anyhow::bail!("絵文字画像がサイズ上限を超えています");
                    }
                    let metadata = image_metadata(response.headers());
                    match response.bytes().await {
                        Ok(bytes) if bytes.len() as u64 <= MAX_IMAGE_SIZE => {
                            return Ok(DownloadedImage {
                                bytes: bytes.to_vec(),
                                metadata,
                            });
                        }
                        Ok(_) => anyhow::bail!("絵文字画像がサイズ上限を超えています"),
                        Err(error) => (Err(error.into()), true),
                    }
                } else {
                    let should_retry = status.as_u16() == 408
                        || status.as_u16() == 429
                        || status.is_server_error();
                    let error = response
                        .error_for_status()
                        .err()
                        .map(anyhow::Error::from)
                        .unwrap_or_else(|| {
                            anyhow::anyhow!("画像サーバーがHTTP {status}を返しました")
                        });
                    (Err(error), should_retry)
                }
            }
        };

        if !should_retry || attempt + 1 == MAX_ATTEMPTS {
            return result;
        }
        tracing::warn!(
            attempt = attempt + 1,
            wait_ms = delay.as_millis(),
            "絵文字画像の取得を再試行します"
        );
        tokio::time::sleep(delay).await;
        delay *= 2;
    }
    Err(anyhow::anyhow!("画像取得の再試行が上限に達しました"))
}

#[derive(Clone)]
struct ImageMetadata {
    etag: Option<String>,
    last_modified: Option<String>,
}

struct DownloadedImage {
    bytes: Vec<u8>,
    metadata: ImageMetadata,
}

enum ImageLoadOutcome {
    Downloaded,
    Skipped,
}

fn image_metadata(headers: &reqwest::header::HeaderMap) -> ImageMetadata {
    let header_value = |name| {
        headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
    };
    ImageMetadata {
        etag: header_value(reqwest::header::ETAG),
        last_modified: header_value(reqwest::header::LAST_MODIFIED),
    }
}

async fn head_image(
    client: &reqwest::Client,
    image_url: &str,
) -> anyhow::Result<Option<ImageMetadata>> {
    let response = client
        .head(image_url)
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await?;
    if response.status() == reqwest::StatusCode::METHOD_NOT_ALLOWED
        || response.status() == reqwest::StatusCode::NOT_IMPLEMENTED
    {
        return Ok(None);
    }
    let response = response.error_for_status()?;
    Ok(Some(image_metadata(response.headers())))
}

fn image_is_unchanged(cached: &database::CachedImage, head: &ImageMetadata) -> bool {
    cached.etag.is_some() && cached.etag == head.etag
        || cached.last_modified.is_some() && cached.last_modified == head.last_modified
}

async fn load_image_hash(
    client: &reqwest::Client,
    image_url: &str,
    cached: Option<&database::CachedImage>,
) -> anyhow::Result<(String, ImageMetadata, ImageLoadOutcome)> {
    if let Some(cached) = cached {
        if let Some(hash) = &cached.hash {
            if let Some(head) = head_image(client, image_url).await? {
                if image_is_unchanged(cached, &head) {
                    return Ok((hash.clone(), head, ImageLoadOutcome::Skipped));
                }
            }
        }
    }
    let image = fetch_image_with_backoff(client, image_url).await?;
    let hash = image_hash::image_hash(&image.bytes)?;
    Ok((hash, image.metadata, ImageLoadOutcome::Downloaded))
}

#[tauri::command]
async fn fetch_server_emojis(app: tauri::AppHandle, id: i64) -> Result<FetchSummary, String> {
    let server_url = database::server_url(&app, id)?;

    tracing::info!(server_id = id, "絵文字の取得を開始します");
    report_fetch_progress(&app, id, "listing", 0, 0);
    let client = metol::Client::with_endpoints(server_url.clone(), String::new(), None)
        .configure_http_api_backoff(8, std::time::Duration::from_secs(1), 2);

    let response = client
        .get_emojis(metol::mi_api_types::emojis::Request::default())
        .await
        .map_err(|e| format!("サーバーから絵文字一覧を取得できません: {e}"))?;
    let total = response.emojis.len();
    tracing::info!(
        server_id = id,
        emoji_count = total,
        "絵文字一覧を取得しました。画像の取得とハッシュ化を開始します"
    );
    report_fetch_progress(&app, id, "images", 0, total);
    let cached_images = database::cached_images(&app, &server_url)?;
    let image_client = reqwest::Client::new();
    let completed = Arc::new(AtomicUsize::new(0));
    let successful_count = Arc::new(AtomicUsize::new(0));
    let skipped_count = Arc::new(AtomicUsize::new(0));
    let failed_count = Arc::new(AtomicUsize::new(0));
    let progress_step = total.div_ceil(10).max(1);
    let items = stream::iter(response.emojis.into_iter().map(|emoji| {
        let client = &client;
        let image_client = image_client.clone();
        let completed = Arc::clone(&completed);
        let successful_count = Arc::clone(&successful_count);
        let skipped_count = Arc::clone(&skipped_count);
        let failed_count = Arc::clone(&failed_count);
        let app = app.clone();
        let cached_image = cached_images.get(&emoji.name).cloned();
        async move {
            let license = match client
                .get_emoji(metol::mi_api_types::emoji::Request {
                    name: emoji.name.clone(),
                })
                .await
            {
                Ok(detail) => detail.license,
                Err(error) => {
                    tracing::error!(
                        emoji = %emoji.name,
                        error = %error,
                        "ライセンス情報を取得できませんでした。NULLとして保存します"
                    );
                    None
                }
            };
            let image_url = match url::Url::parse(&emoji.url) {
                Ok(url) if matches!(url.scheme(), "http" | "https") => Some(url.to_string()),
                Ok(_) => {
                    tracing::error!(emoji = %emoji.name, "絵文字画像URLがHTTP(S)ではありません");
                    None
                }
                Err(error) => {
                    tracing::error!(emoji = %emoji.name, error = %error, "絵文字画像URLを解析できません");
                    None
                }
            };
            let (hash, etag, last_modified) = match &image_url {
                Some(image_url) => match load_image_hash(
                    &image_client,
                    image_url,
                    cached_image.as_ref(),
                )
                .await
                {
                    Ok((hash, metadata, outcome)) => {
                        match outcome {
                            ImageLoadOutcome::Downloaded => {
                                successful_count.fetch_add(1, Ordering::Relaxed);
                            }
                            ImageLoadOutcome::Skipped => {
                                skipped_count.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                        (Some(hash), metadata.etag, metadata.last_modified)
                    }
                    Err(error) => {
                        failed_count.fetch_add(1, Ordering::Relaxed);
                        tracing::error!(
                            emoji = %emoji.name,
                            error = %error,
                            "画像の取得またはハッシュ計算に失敗しました"
                        );
                        (None, None, None)
                    }
                },
                None => {
                    failed_count.fetch_add(1, Ordering::Relaxed);
                    (None, None, None)
                }
            };
            let done = completed.fetch_add(1, Ordering::Relaxed) + 1;
            report_fetch_progress(&app, id, "images", done, total);
            if done % progress_step == 0 || done == total {
                let percent = done * 100 / total.max(1);
                tracing::info!(server_id = id, completed = done, total, percent, "絵文字画像処理の進捗");
            }
            database::EmojiRecord {
                name: emoji.name,
                category: emoji.category,
                aliases: emoji.aliases,
                license,
                image_url,
                hash,
                etag,
                last_modified,
            }
        }
    }))
    .buffer_unordered(8)
    .collect::<Vec<_>>()
    .await;
    let emoji_count = items.len();
    let image_count = items.iter().filter(|item| item.hash.is_some()).count();
    let successful_count = successful_count.load(Ordering::Relaxed);
    let skipped_count = skipped_count.load(Ordering::Relaxed);
    let failed_count = failed_count.load(Ordering::Relaxed);
    tracing::info!(
        server_id = id,
        image_count,
        successful_count,
        skipped_count,
        failed_count,
        "画像処理が完了しました。SQLiteへ保存します"
    );
    report_fetch_progress(&app, id, "saving", total, total);

    database::replace_server_emojis(&app, id, &server_url, &items)?;
    tracing::info!(
        server_id = id,
        emoji_count,
        image_count,
        "絵文字データの保存が完了しました"
    );
    report_fetch_progress(&app, id, "complete", total, total);
    Ok(FetchSummary {
        emoji_count,
        image_count,
    })
}

#[tauri::command]
async fn calculate_image_hash(image: Vec<u8>) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        image_hash::image_hash(&image)
            .map_err(|error| format!("画像ハッシュを計算できません: {error}"))
    })
    .await
    .map_err(|error| error.to_string())?
}

async fn load_comparison_source(source: ComparisonSource) -> Result<Vec<u8>, String> {
    if let Some(image) = source.image {
        return Ok(image);
    }
    let url = source
        .url
        .filter(|url| !url.trim().is_empty())
        .ok_or_else(|| "画像ファイルまたは URL を指定してください".to_string())?;
    let parsed = url::Url::parse(&url).map_err(|error| format!("URL が不正です: {error}"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("URL は http または https で指定してください".to_string());
    }
    let response = reqwest::Client::new()
        .get(parsed)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|error| format!("画像 URL の取得に失敗しました: {error}"))?
        .error_for_status()
        .map_err(|error| format!("画像 URL の取得に失敗しました: {error}"))?;
    response
        .bytes()
        .await
        .map(|bytes| bytes.to_vec())
        .map_err(|error| format!("画像データの読み込みに失敗しました: {error}"))
}

fn encode_comparison_grid(grid: &ndarray::Array2<f32>) -> anyhow::Result<String> {
    use base64::{Engine, engine::general_purpose::STANDARD};
    use image::ImageEncoder;

    let (height, width) = grid.dim();
    let min = grid.iter().copied().fold(f32::INFINITY, f32::min);
    let max = grid.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let range = (max - min).max(f32::EPSILON);
    let pixels = grid
        .iter()
        .map(|value| ((value - min) / range * 255.0).round() as u8)
        .collect::<Vec<_>>();
    let mut encoded = Vec::new();
    image::codecs::png::PngEncoder::new(&mut encoded).write_image(
        &pixels,
        width as u32,
        height as u32,
        image::ExtendedColorType::L8,
    )?;
    Ok(STANDARD.encode(encoded))
}

fn encode_grid_difference(
    left: &ndarray::Array2<f32>,
    right: &ndarray::Array2<f32>,
) -> anyhow::Result<String> {
    use base64::{Engine, engine::general_purpose::STANDARD};
    use image::ImageEncoder;

    if left.dim() != right.dim() {
        anyhow::bail!("グリッドの寸法が一致しません");
    }
    let (height, width) = left.dim();
    let pixels = left
        .iter()
        .zip(right.iter())
        .flat_map(|(left, right)| {
            let difference = left - right;
            let intensity = (difference.abs().clamp(0.0, 1.0) * 255.0).round() as u8;
            if difference > 0.0 {
                [intensity, 0, 0, 255]
            } else {
                [0, intensity, 0, 255]
            }
        })
        .collect::<Vec<_>>();
    let mut encoded = Vec::new();
    image::codecs::png::PngEncoder::new(&mut encoded).write_image(
        &pixels,
        width as u32,
        height as u32,
        image::ExtendedColorType::Rgba8,
    )?;
    Ok(STANDARD.encode(encoded))
}

fn encode_hash_visualization(panel: &ndarray::Array2<bool>) -> anyhow::Result<String> {
    use base64::{Engine, engine::general_purpose::STANDARD};
    use image::ImageEncoder;

    let (height, width) = panel.dim();
    let pixels = panel
        .iter()
        .map(|bit| if *bit { 0 } else { 255 })
        .collect::<Vec<_>>();
    let mut encoded = Vec::new();
    image::codecs::png::PngEncoder::new(&mut encoded).write_image(
        &pixels,
        width as u32,
        height as u32,
        image::ExtendedColorType::L8,
    )?;
    Ok(STANDARD.encode(encoded))
}

fn original_image_data_url(image: &[u8]) -> anyhow::Result<String> {
    use base64::{Engine, engine::general_purpose::STANDARD};

    let content_type = match image::guess_format(image)? {
        image::ImageFormat::Png => "image/png",
        image::ImageFormat::Jpeg => "image/jpeg",
        image::ImageFormat::Gif => "image/gif",
        image::ImageFormat::WebP => "image/webp",
        _ => "application/octet-stream",
    };
    Ok(format!(
        "data:{content_type};base64,{}",
        STANDARD.encode(image)
    ))
}

#[tauri::command]
async fn compare_images(
    left: ComparisonSource,
    right: ComparisonSource,
) -> Result<ImageComparison, String> {
    let (left, right) =
        futures_util::try_join!(load_comparison_source(left), load_comparison_source(right))?;
    tauri::async_runtime::spawn_blocking(move || {
        let left_data = image_hash::image_hash_data(&left)
            .map_err(|error| format!("左画像を解析できません: {error}"))?;
        let right_data = image_hash::image_hash_data(&right)
            .map_err(|error| format!("右画像を解析できません: {error}"))?;
        let image_hash::ImageHashData {
            hash: left_hash,
            grids: left_grids,
            visualization: left_hash_visualization,
        } = left_data;
        let image_hash::ImageHashData {
            hash: right_hash,
            grids: right_grids,
            visualization: right_hash_visualization,
        } = right_data;
        let left_frames = left_grids
            .iter()
            .map(encode_comparison_grid)
            .collect::<anyhow::Result<Vec<_>>>()
            .map_err(|error| format!("左画像のグリッドを PNG に変換できません: {error}"))?;
        let right_frames = right_grids
            .iter()
            .map(encode_comparison_grid)
            .collect::<anyhow::Result<Vec<_>>>()
            .map_err(|error| format!("右画像のグリッドを PNG に変換できません: {error}"))?;
        let difference_frames = (left_grids.len() == right_grids.len())
            .then(|| {
                left_grids
                    .iter()
                    .zip(right_grids.iter())
                    .map(|(left, right)| encode_grid_difference(left, right))
                    .collect::<anyhow::Result<Vec<_>>>()
            })
            .transpose()
            .map_err(|error| format!("グリッドの差分を PNG に変換できません: {error}"))?;
        let left_hash_visualization = left_hash_visualization
            .iter()
            .map(encode_hash_visualization)
            .collect::<anyhow::Result<Vec<_>>>()
            .map_err(|error| format!("左画像のハッシュを可視化できません: {error}"))?;
        let right_hash_visualization = right_hash_visualization
            .iter()
            .map(encode_hash_visualization)
            .collect::<anyhow::Result<Vec<_>>>()
            .map_err(|error| format!("右画像のハッシュを可視化できません: {error}"))?;
        Ok(ImageComparison {
            distance: image_hash::distance(&left_hash, &right_hash),
            left: ComparisonImage {
                original: original_image_data_url(&left)
                    .map_err(|error| format!("左画像を表示用に変換できません: {error}"))?,
                frames: left_frames,
                hash: left_hash,
                hash_visualization: left_hash_visualization,
            },
            right: ComparisonImage {
                original: original_image_data_url(&right)
                    .map_err(|error| format!("右画像を表示用に変換できません: {error}"))?,
                frames: right_frames,
                hash: right_hash,
                hash_visualization: right_hash_visualization,
            },
            difference_frames,
        })
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
fn match_hash(
    app: tauri::AppHandle,
    hash: String,
    threshold: u32,
) -> Result<Vec<database::EmojiMatch>, String> {
    let matches = database::find_emoji_matches(&app, &hash, threshold)?;
    if matches.is_empty() {
        tracing::info!(query_hash = %hash, "画像照合で一致する候補はありませんでした");
    } else {
        for item in &matches {
            tracing::info!(
                query_hash = %hash,
                matched_hash = %item.hash,
                server = %item.host,
                emoji = %item.name,
                distance = item.distance,
                "画像照合で絵文字候補に一致しました"
            );
        }
    }
    Ok(matches)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .try_init();
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let version = database::database_version(app.handle())?;
            if version.as_deref() != Some(database::DATABASE_VERSION) {
                let message = match version {
                    Some(version) => format!(
                        "データベースのバージョンが異なります。\n現在: {version}\n必要: {}\n\nデータベースを初期化しますか？",
                        database::DATABASE_VERSION
                    ),
                    None => format!(
                        "データベースのバージョン情報がありません。\n必要: {}\n\nデータベースを初期化しますか？",
                        database::DATABASE_VERSION
                    ),
                };
                let initialize = app
                    .dialog()
                    .message(message)
                    .title("データベースの更新")
                    .buttons(MessageDialogButtons::YesNo)
                    .blocking_show();
                if initialize {
                    database::reset_database(app.handle())?;
                } else {
                    app.handle().exit(0);
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            export_emoji_package,
            import_emoji_package,
            list_servers,
            database::list_server_emojis,
            database::rebuild_hash_parts,
            add_server,
            remove_server,
            database::set_server_favorite,
            fetch_server_emojis,
            calculate_image_hash,
            compare_images,
            match_hash
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
