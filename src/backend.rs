//! Tauri バックエンド（`src-tauri`）のコマンド呼び出しラッパーと、バックエンドと共有する型。

use dioxus::prelude::*;
use js_sys::{Function, Object, Promise, Reflect};
use serde::{Deserialize, de::DeserializeOwned};
use std::collections::HashMap;
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_futures::JsFuture;

#[derive(Clone, Deserialize)]
pub struct Server {
    pub id: i64,
    pub url: String,
    pub emoji_count: i64,
    pub last_fetched_at: Option<String>,
    pub favorite: bool,
}

#[derive(Clone, Deserialize, PartialEq)]
pub struct EmojiMatch {
    pub name: String,
    pub category: Option<String>,
    pub aliases: Vec<String>,
    pub license: Option<String>,
    pub host: String,
    pub distance: u32,
}

#[derive(Clone, Deserialize, PartialEq)]
pub struct ServerEmoji {
    pub name: String,
    pub category: Option<String>,
    pub aliases: Vec<String>,
    pub license: Option<String>,
    pub image_url: Option<String>,
}

#[derive(Deserialize)]
pub struct FetchSummary {
    pub emoji_count: usize,
    pub image_count: usize,
}

#[derive(Clone, Deserialize)]
pub struct FetchProgress {
    pub server_id: i64,
    pub stage: String,
    pub completed: usize,
    pub total: usize,
}

impl FetchProgress {
    pub fn listing(server_id: i64) -> Self {
        Self {
            server_id,
            stage: "listing".to_string(),
            completed: 0,
            total: 0,
        }
    }

    pub fn percent(&self) -> usize {
        if self.total == 0 {
            0
        } else {
            self.completed * 100 / self.total
        }
    }
}

#[derive(Clone, Deserialize, PartialEq)]
pub struct ComparisonImage {
    pub original: String,
    pub frames: Vec<String>,
    pub hash: String,
    pub hash_visualization: Vec<String>,
}

#[derive(Clone, Deserialize, PartialEq)]
pub struct ImageComparison {
    pub left: ComparisonImage,
    pub right: ComparisonImage,
    pub difference_frames: Option<Vec<String>>,
    pub distance: u32,
}

#[derive(Deserialize)]
pub struct ExportSummary {
    pub path: String,
    pub emoji_count: usize,
}

pub struct ExportEmoji {
    pub file_name: String,
    pub emoji_name: String,
    pub category: String,
    pub aliases: Vec<String>,
    pub license: String,
    pub image: Vec<u8>,
}

#[derive(Deserialize)]
pub struct ImportedEmoji {
    pub file_name: String,
    pub emoji_name: String,
    pub category: String,
    pub aliases: Vec<String>,
    pub license: String,
    pub mime: String,
    pub image: Vec<u8>,
}

#[derive(Deserialize)]
pub struct ImportedPackage {
    pub emojis: Vec<ImportedEmoji>,
    pub warnings: Vec<String>,
}

async fn invoke<T: DeserializeOwned>(command: &str, args: JsValue) -> Result<T, String> {
    let window = web_sys::window().ok_or_else(|| "アプリに接続できません".to_string())?;
    let tauri = Reflect::get(&window, &"__TAURI__".into()).map_err(|e| format!("{e:?}"))?;
    let core = Reflect::get(&tauri, &"core".into()).map_err(|e| format!("{e:?}"))?;
    let function: Function = Reflect::get(&core, &"invoke".into())
        .map_err(|e| format!("{e:?}"))?
        .dyn_into()
        .map_err(|e| format!("{e:?}"))?;
    let promise: Promise = function
        .call2(&core, &command.into(), &args)
        .map_err(|e| format!("{e:?}"))?
        .dyn_into()
        .map_err(|e| format!("{e:?}"))?;
    let value = JsFuture::from(promise)
        .await
        .map_err(|e| format!("{e:?}"))?;
    serde_wasm_bindgen::from_value(value).map_err(|e| e.to_string())
}

fn invoke_args(key: &str, value: JsValue) -> JsValue {
    let args = Object::new();
    let _ = Reflect::set(&args, &key.into(), &value);
    args.into()
}

fn bytes_to_js(bytes: &[u8]) -> js_sys::Array {
    js_sys::Array::from(&js_sys::Uint8Array::from(bytes))
}

pub async fn list_servers() -> Result<Vec<Server>, String> {
    invoke("list_servers", Object::new().into()).await
}

pub async fn list_server_emojis(host: String) -> Result<Vec<ServerEmoji>, String> {
    invoke("list_server_emojis", invoke_args("host", host.into())).await
}

pub async fn add_server(url: String) -> Result<bool, String> {
    invoke("add_server", invoke_args("url", url.into())).await
}

pub async fn remove_server(id: i64) -> Result<bool, String> {
    invoke(
        "remove_server",
        invoke_args("id", JsValue::from_f64(id as f64)),
    )
    .await
}

pub async fn set_server_favorite(id: i64, favorite: bool) -> Result<bool, String> {
    let args = Object::new();
    let _ = Reflect::set(&args, &"id".into(), &JsValue::from_f64(id as f64));
    let _ = Reflect::set(&args, &"favorite".into(), &JsValue::from_bool(favorite));
    invoke("set_server_favorite", args.into()).await
}

pub async fn fetch_server_emojis(id: i64) -> Result<FetchSummary, String> {
    invoke(
        "fetch_server_emojis",
        invoke_args("id", JsValue::from_f64(id as f64)),
    )
    .await
}

pub async fn rebuild_hash_parts() -> Result<usize, String> {
    invoke("rebuild_hash_parts", Object::new().into()).await
}

pub async fn subscribe_fetch_progress(mut progress: Signal<HashMap<i64, FetchProgress>>) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(events) = Reflect::get(&window, &"__TAURI__".into())
        .and_then(|tauri| Reflect::get(&tauri, &"event".into()))
    else {
        return;
    };
    let Ok(listen) = Reflect::get(&events, &"listen".into()) else {
        return;
    };
    let Ok(listen) = listen.dyn_into::<Function>() else {
        return;
    };

    let callback = Closure::<dyn FnMut(JsValue)>::new(move |event| {
        let Ok(payload) = Reflect::get(&event, &"payload".into()) else {
            return;
        };
        if let Ok(update) = serde_wasm_bindgen::from_value::<FetchProgress>(payload) {
            progress.with_mut(|updates| {
                updates.insert(update.server_id, update);
            });
        }
    });
    let Ok(promise) = listen
        .call2(&events, &"emoji-fetch-progress".into(), callback.as_ref())
        .and_then(|result| result.dyn_into::<Promise>().map_err(|error| error))
    else {
        return;
    };
    if JsFuture::from(promise).await.is_ok() {
        callback.forget();
    }
}

pub async fn calculate_image_hash(bytes: Vec<u8>) -> Result<String, String> {
    let args = Object::new();
    let _ = Reflect::set(&args, &"image".into(), &bytes_to_js(&bytes));
    invoke("calculate_image_hash", args.into()).await
}

fn comparison_source(image: Option<Vec<u8>>, url: String) -> JsValue {
    let source = Object::new();
    if let Some(image) = image {
        let _ = Reflect::set(&source, &"image".into(), &bytes_to_js(&image));
    } else {
        let _ = Reflect::set(&source, &"image".into(), &JsValue::NULL);
    }
    let _ = Reflect::set(&source, &"url".into(), &url.into());
    source.into()
}

pub async fn compare_images(
    left_image: Option<Vec<u8>>,
    left_url: String,
    right_image: Option<Vec<u8>>,
    right_url: String,
) -> Result<ImageComparison, String> {
    let args = Object::new();
    let _ = Reflect::set(
        &args,
        &"left".into(),
        &comparison_source(left_image, left_url),
    );
    let _ = Reflect::set(
        &args,
        &"right".into(),
        &comparison_source(right_image, right_url),
    );
    invoke("compare_images", args.into()).await
}

pub async fn match_hash(hash: String, threshold: u32) -> Result<Vec<EmojiMatch>, String> {
    let args = Object::new();
    let _ = Reflect::set(&args, &"hash".into(), &hash.into());
    let _ = Reflect::set(
        &args,
        &"threshold".into(),
        &JsValue::from_f64(threshold as f64),
    );
    invoke("match_hash", args.into()).await
}

pub async fn import_emoji_package(archive: Vec<u8>) -> Result<ImportedPackage, String> {
    let args = Object::new();
    let _ = Reflect::set(&args, &"archive".into(), &bytes_to_js(&archive));
    invoke("import_emoji_package", args.into()).await
}

pub async fn export_emoji_package(
    emojis: Vec<ExportEmoji>,
) -> Result<Option<ExportSummary>, String> {
    let items = js_sys::Array::new();
    for emoji in emojis {
        let item = Object::new();
        let aliases = js_sys::Array::new();
        for alias in &emoji.aliases {
            aliases.push(&alias.into());
        }
        let _ = Reflect::set(&item, &"file_name".into(), &emoji.file_name.into());
        let _ = Reflect::set(&item, &"emoji_name".into(), &emoji.emoji_name.into());
        let _ = Reflect::set(&item, &"category".into(), &emoji.category.into());
        let _ = Reflect::set(&item, &"aliases".into(), &aliases);
        let _ = Reflect::set(&item, &"license".into(), &emoji.license.into());
        let _ = Reflect::set(&item, &"image".into(), &bytes_to_js(&emoji.image));
        items.push(&item);
    }
    invoke("export_emoji_package", invoke_args("emojis", items.into())).await
}
