use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::io::{Cursor, Read};
use zip::ZipArchive;

const MAX_META_SIZE: u64 = 64 * 1024 * 1024;
const MAX_IMAGE_SIZE: u64 = 16 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackageMeta {
    emojis: Vec<PackageEmoji>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackageEmoji {
    file_name: String,
    emoji: PackageEmojiInfo,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackageEmojiInfo {
    name: String,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(default)]
    license: Option<String>,
}

#[derive(Serialize)]
pub struct ImportedEmoji {
    pub file_name: String,
    pub emoji_name: String,
    pub category: String,
    pub aliases: Vec<String>,
    pub license: String,
    pub mime: String,
    pub image: Vec<u8>,
}

#[derive(Serialize)]
pub struct ImportedPackage {
    pub emojis: Vec<ImportedEmoji>,
    /// 読み込めなかった項目の理由。
    pub warnings: Vec<String>,
}

fn read_entry(
    archive: &mut ZipArchive<Cursor<Vec<u8>>>,
    name: &str,
    limit: u64,
) -> Result<Vec<u8>> {
    let entry = archive.by_name(name)?;
    let mut bytes = Vec::new();
    entry.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        bail!("サイズが上限を超えています");
    }
    Ok(bytes)
}

/// `export::build_package` や Misskey が書き出す形式（meta.json + 画像）の ZIP を読み込む。
pub fn read_package(archive: Vec<u8>) -> Result<ImportedPackage> {
    let mut archive =
        ZipArchive::new(Cursor::new(archive)).context("ZIP ファイルとして読み込めません")?;
    let meta = read_entry(&mut archive, "meta.json", MAX_META_SIZE)
        .context("meta.json を読み込めません")?;
    let meta: PackageMeta =
        serde_json::from_slice(&meta).context("meta.json の形式が正しくありません")?;

    let mut emojis = Vec::with_capacity(meta.emojis.len());
    let mut warnings = Vec::new();
    for entry in meta.emojis {
        let label = if entry.emoji.name.is_empty() {
            entry.file_name.as_str()
        } else {
            entry.emoji.name.as_str()
        };
        let image = match read_entry(&mut archive, &entry.file_name, MAX_IMAGE_SIZE) {
            Ok(image) => image,
            Err(error) => {
                warnings.push(format!("{label}: {} を読み込めません ({error})", entry.file_name));
                continue;
            }
        };
        let Ok(format) = image::guess_format(&image) else {
            warnings.push(format!("{label}: 画像形式を判別できません"));
            continue;
        };

        emojis.push(ImportedEmoji {
            file_name: entry.file_name,
            emoji_name: entry.emoji.name,
            category: entry.emoji.category.unwrap_or_default(),
            aliases: entry.emoji.aliases,
            license: entry.emoji.license.unwrap_or_default(),
            mime: format.to_mime_type().to_owned(),
            image,
        });
    }

    Ok(ImportedPackage { emojis, warnings })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::export::{ExportEmoji, build_package, prepare};

    #[test]
    fn reads_exported_package() {
        let mut image = Vec::new();
        image::DynamicImage::new_rgba8(2, 2)
            .write_to(&mut Cursor::new(&mut image), image::ImageFormat::Png)
            .unwrap();
        let prepared = prepare(vec![ExportEmoji {
            file_name: "a.png".into(),
            emoji_name: "nyan".into(),
            category: "Cat".into(),
            aliases: vec!["neko".into()],
            license: "CC0".into(),
            image: image.clone(),
        }])
        .unwrap();
        let package = read_package(build_package(&prepared).unwrap()).unwrap();
        assert!(package.warnings.is_empty());
        let emoji = &package.emojis[0];
        assert_eq!(emoji.emoji_name, "nyan");
        assert_eq!(emoji.file_name, "nyan.png");
        assert_eq!(emoji.category, "Cat");
        assert_eq!(emoji.aliases, vec!["neko".to_string()]);
        assert_eq!(emoji.license, "CC0");
        assert_eq!(emoji.mime, "image/png");
        assert_eq!(emoji.image, image);
    }

    #[test]
    fn rejects_non_package() {
        assert!(read_package(b"not a zip".to_vec()).is_err());
    }
}
