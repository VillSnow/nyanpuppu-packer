use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::{Cursor, Write};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

const PACKAGE_HOST: &str = "nyanpuppu-packer";

#[derive(Deserialize)]
pub struct ExportEmoji {
    pub file_name: String,
    pub emoji_name: String,
    pub category: String,
    pub aliases: Vec<String>,
    pub license: String,
    pub image: Vec<u8>,
}

pub struct PreparedEmoji {
    archive_file_name: String,
    name: String,
    category: Option<String>,
    aliases: Vec<String>,
    license: Option<String>,
    image: Vec<u8>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PackageMeta<'a> {
    meta_version: u32,
    host: &'a str,
    exported_at: String,
    emojis: Vec<PackageEmoji<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PackageEmoji<'a> {
    downloaded: bool,
    file_name: &'a str,
    emoji: PackageEmojiInfo<'a>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PackageEmojiInfo<'a> {
    name: &'a str,
    category: Option<&'a str>,
    aliases: &'a [String],
    license: Option<&'a str>,
    local_only: bool,
    is_sensitive: bool,
    role_ids_that_can_be_used_this_emoji_as_reaction: [&'a str; 0],
}

fn is_valid_emoji_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn image_extension(image: &[u8]) -> Option<&'static str> {
    image::guess_format(image)
        .ok()?
        .extensions_str()
        .first()
        .copied()
}

/// 書き出し前に全件を検証し、パッケージに入れる内容を確定する。
/// 問題のある項目はまとめて報告する。
pub fn prepare(emojis: Vec<ExportEmoji>) -> Result<Vec<PreparedEmoji>> {
    if emojis.is_empty() {
        bail!("書き出す画像がありません");
    }

    let mut problems = Vec::new();
    let mut seen_names = HashSet::new();
    let mut prepared = Vec::with_capacity(emojis.len());

    for emoji in emojis {
        let name = emoji.emoji_name.trim().to_owned();
        let label = &emoji.file_name;
        if name.is_empty() {
            problems.push(format!("{label}: 絵文字名が未入力です"));
            continue;
        }
        if !is_valid_emoji_name(&name) {
            problems.push(format!(
                "{label}: 絵文字名「{name}」には英数字とアンダースコアのみ使用できます"
            ));
            continue;
        }
        if !seen_names.insert(name.clone()) {
            problems.push(format!("{label}: 絵文字名「{name}」が重複しています"));
            continue;
        }
        let Some(extension) = image_extension(&emoji.image) else {
            problems.push(format!("{label}: 画像形式を判別できません"));
            continue;
        };

        let mut aliases = Vec::new();
        for alias in emoji.aliases {
            let alias = alias.trim().to_owned();
            if !alias.is_empty() && !aliases.contains(&alias) {
                aliases.push(alias);
            }
        }
        let category = emoji.category.trim();
        let license = emoji.license.trim();

        prepared.push(PreparedEmoji {
            archive_file_name: format!("{name}.{extension}"),
            name,
            category: (!category.is_empty()).then(|| category.to_owned()),
            aliases,
            license: (!license.is_empty()).then(|| license.to_owned()),
            image: emoji.image,
        });
    }

    if !problems.is_empty() {
        bail!("書き出せない項目があります:\n{}", problems.join("\n"));
    }
    Ok(prepared)
}

/// Misskey のカスタム絵文字インポートが読める形式（meta.json + 画像）の ZIP を生成する。
pub fn build_package(emojis: &[PreparedEmoji]) -> Result<Vec<u8>> {
    let meta = PackageMeta {
        meta_version: 2,
        host: PACKAGE_HOST,
        exported_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        emojis: emojis
            .iter()
            .map(|emoji| PackageEmoji {
                downloaded: true,
                file_name: &emoji.archive_file_name,
                emoji: PackageEmojiInfo {
                    name: &emoji.name,
                    category: emoji.category.as_deref(),
                    aliases: &emoji.aliases,
                    license: emoji.license.as_deref(),
                    local_only: false,
                    is_sensitive: false,
                    role_ids_that_can_be_used_this_emoji_as_reaction: [],
                },
            })
            .collect(),
    };

    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    writer
        .start_file("meta.json", options)
        .context("meta.json を ZIP に追加できません")?;
    writer.write_all(&serde_json::to_vec_pretty(&meta)?)?;

    // 画像は圧縮済みの形式が多いため、そのまま格納する。
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    for emoji in emojis {
        writer
            .start_file(&emoji.archive_file_name, stored)
            .with_context(|| format!("{} を ZIP に追加できません", emoji.archive_file_name))?;
        writer.write_all(&emoji.image)?;
    }

    Ok(writer.finish()?.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn png() -> Vec<u8> {
        let mut bytes = Vec::new();
        image::DynamicImage::new_rgba8(2, 2)
            .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
            .unwrap();
        bytes
    }

    fn emoji(name: &str) -> ExportEmoji {
        ExportEmoji {
            file_name: "a.png".into(),
            emoji_name: name.into(),
            category: " Cat ".into(),
            aliases: vec![" neko ".into(), "neko".into(), "".into()],
            license: " CC0 ".into(),
            image: png(),
        }
    }

    #[test]
    fn builds_misskey_package() {
        let prepared = prepare(vec![emoji("nyan")]).unwrap();
        let archive = build_package(&prepared).unwrap();
        let mut zip = zip::ZipArchive::new(Cursor::new(archive)).unwrap();
        let mut meta = String::new();
        zip.by_name("meta.json").unwrap().read_to_string(&mut meta).unwrap();
        let meta: serde_json::Value = serde_json::from_str(&meta).unwrap();
        assert_eq!(meta["emojis"][0]["fileName"], "nyan.png");
        assert_eq!(meta["emojis"][0]["emoji"]["aliases"], serde_json::json!(["neko"]));
        assert_eq!(meta["emojis"][0]["emoji"]["category"], "Cat");
        assert_eq!(meta["emojis"][0]["emoji"]["license"], "CC0");
        assert!(zip.by_name("nyan.png").is_ok());
    }

    #[test]
    fn rejects_invalid_names() {
        assert!(prepare(vec![emoji("")]).is_err());
        assert!(prepare(vec![emoji("日本語")]).is_err());
        assert!(prepare(vec![emoji("a"), emoji("a")]).is_err());
    }
}
