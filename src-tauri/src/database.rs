use base64::Engine;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use tauri::Manager;

pub const DATABASE_VERSION: &str = "2026-10-13";

#[derive(Serialize)]
pub struct Server {
    id: i64,
    url: String,
    emoji_count: i64,
    last_fetched_at: Option<String>,
}

pub struct EmojiRecord {
    pub name: String,
    pub category: Option<String>,
    pub aliases: Vec<String>,
    pub license: Option<String>,
    pub image_url: Option<String>,
    pub hash: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

#[derive(Clone)]
pub struct CachedImage {
    pub hash: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

#[derive(Serialize)]
pub struct EmojiMatch {
    pub name: String,
    pub hash: String,
    pub category: Option<String>,
    pub aliases: Vec<String>,
    pub license: Option<String>,
    pub host: String,
    pub distance: u32,
}

pub fn find_emoji_matches(
    app: &tauri::AppHandle,
    hash: &str,
    threshold: u32,
) -> Result<Vec<EmojiMatch>, String> {
    let connection = open_database(app)?;
    // Both partition sizes can share positions: matching compares the full
    // (position, part) pair, and the 12-bit and 24-bit part strings differ in
    // width.
    let parts_12 = split_hash_parts(hash, 12)?;
    let parts_24 = split_hash_parts(hash, 24)?;
    let parts = if (threshold as usize) < parts_24.len() {
        parts_24
    } else {
        parts_12
    };
    if threshold as usize >= parts.len() {
        return Err(format!(
            "閾値 {threshold} は hash_parts による高速検索の上限（{}）を超えています",
            parts.len().saturating_sub(1)
        ));
    }
    let required_matching_parts = parts.len() - threshold as usize;
    let values = parts
        .iter()
        .map(|_| "(?, ?)")
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "with query_parts(position, part) as (values {values})
         select e.host, e.hash, e.name, e.category, e.aliases, e.license
         from query_parts q join hash_parts hp on hp.position = q.position and hp.part = q.part
         join emojis e on e.hash = hp.hash
         where length(e.hash) = ?
         group by e.host, e.hash, e.name, e.category, e.aliases, e.license
         having count(distinct hp.position) >= ?"
    );
    let mut params = Vec::with_capacity(parts.len() * 2 + 2);
    for (position, part) in parts {
        params.push(rusqlite::types::Value::Integer(position as i64));
        params.push(rusqlite::types::Value::Text(part));
    }
    params.push(rusqlite::types::Value::Integer(hash.len() as i64));
    params.push(rusqlite::types::Value::Integer(
        required_matching_parts as i64,
    ));
    let mut statement = connection.prepare(&sql).map_err(|e| e.to_string())?;
    let candidates = statement
        .query_map(rusqlite::params_from_iter(params), |row| {
            let aliases_json: String = row.get(4)?;
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                serde_json::from_str::<Vec<String>>(&aliases_json).unwrap_or_default(),
                row.get::<_, Option<String>>(5)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let mut best_by_name: HashMap<(String, String), EmojiMatch> = HashMap::new();
    for (host, candidate_hash, name, category, mut aliases, license) in candidates {
        if name.trim().is_empty() {
            continue;
        }
        aliases.retain(|alias| !alias.trim().is_empty());
        let distance = crate::image_hash::distance(hash, &candidate_hash);
        if distance <= threshold {
            let key = (host.clone(), name.clone());
            let candidate = EmojiMatch {
                name,
                hash: candidate_hash,
                category,
                aliases,
                license,
                host,
                distance,
            };
            if best_by_name
                .get(&key)
                .is_none_or(|existing| candidate.distance < existing.distance)
            {
                best_by_name.insert(key, candidate);
            }
        }
    }
    let mut matches = best_by_name.into_values().collect::<Vec<_>>();
    matches.sort_by_key(|item| item.distance);
    Ok(matches)
}

fn database_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("nyanpuppu.sqlite3"))
}

fn open_database(app: &tauri::AppHandle) -> Result<Connection, String> {
    let connection = Connection::open(database_path(app)?).map_err(|e| e.to_string())?;
    connection
        .execute_batch(include_str!("init.sql"))
        .map_err(|e| e.to_string())?;
    connection
        .execute(
            "insert or ignore into metadata (key, value) values ('db_version', ?1)",
            params![DATABASE_VERSION],
        )
        .map_err(|e| e.to_string())?;
    ensure_column(&connection, "emojis", "image_url", "text")?;
    ensure_column(&connection, "emojis", "etag", "text")?;
    ensure_column(&connection, "emojis", "last_modified", "text")?;
    ensure_column(&connection, "servers", "last_fetched_at", "text")?;
    Ok(connection)
}

pub fn cached_images(
    app: &tauri::AppHandle,
    host: &str,
) -> Result<HashMap<String, CachedImage>, String> {
    let connection = open_database(app)?;
    let mut statement = connection
        .prepare(
            "select name, hash, etag, last_modified
             from emojis where host = ?1",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(params![host], |row| {
            Ok((
                row.get(0)?,
                CachedImage {
                    hash: row.get(1)?,
                    etag: row.get(2)?,
                    last_modified: row.get(3)?,
                },
            ))
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<HashMap<_, _>, _>>()
        .map_err(|error| error.to_string())
}

pub fn database_version(app: &tauri::AppHandle) -> Result<Option<String>, String> {
    let path = database_path(app)?;
    if !path.exists() {
        return Ok(Some(DATABASE_VERSION.to_string()));
    }
    let connection = Connection::open(path).map_err(|e| e.to_string())?;
    connection
        .execute_batch(include_str!("init.sql"))
        .map_err(|e| e.to_string())?;
    connection
        .query_row(
            "select value from metadata where key = 'db_version'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())
}

pub fn reset_database(app: &tauri::AppHandle) -> Result<(), String> {
    let path = database_path(app)?;
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    open_database(app).map(|_| ())
}

fn split_hash_parts(hash: &str, part_bits: usize) -> Result<Vec<(i64, String)>, String> {
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(hash)
        .map_err(|error| format!("ハッシュをデコードできません: {error}"))?;
    if !matches!(part_bits, 12 | 24) || hash.len() % 4 != 0 || decoded.len() % 3 != 0 {
        return Err("ハッシュの分割形式が不正です".to_string());
    }

    // Base64 characters represent 6 bits each, so 12-bit and 24-bit parts
    // are exactly 2 and 4 characters respectively.
    let part_chars = part_bits / 6;
    let part_count = hash.len().div_ceil(part_chars);
    let mut parts = Vec::with_capacity(part_count);

    for (part_index, part) in hash.as_bytes().chunks(part_chars).enumerate() {
        let part = std::str::from_utf8(part)
            .map_err(|error| format!("ハッシュの分割に失敗しました: {error}"))?;
        parts.push((part_index as i64, part.to_string()));
    }

    Ok(parts)
}

#[tauri::command]
pub fn rebuild_hash_parts(app: tauri::AppHandle) -> Result<usize, String> {
    let connection = open_database(&app)?;
    let hashes = {
        let mut statement = connection
            .prepare("select distinct hash from emojis where hash is not null")
            .map_err(|error| error.to_string())?;
        statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?
    };

    let transaction = connection
        .unchecked_transaction()
        .map_err(|error| error.to_string())?;
    let hash_count = hashes.len();
    transaction
        .execute("delete from hash_parts", [])
        .map_err(|error| error.to_string())?;
    for hash in hashes {
        for part_bits in [12, 24] {
            for (position, part) in split_hash_parts(&hash, part_bits)? {
                transaction
                    .execute(
                        "insert or ignore into hash_parts (hash, position, part) values (?1, ?2, ?3)",
                        (&hash, position, part),
                    )
                    .map_err(|error| error.to_string())?;
            }
        }
    }
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(hash_count)
}

fn ensure_column(
    connection: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<(), String> {
    let mut statement = connection
        .prepare(&format!("pragma table_info({table})"))
        .map_err(|e| e.to_string())?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| e.to_string())?;
    for existing in columns {
        if existing.map_err(|e| e.to_string())? == column {
            return Ok(());
        }
    }
    connection
        .execute_batch(&format!(
            "alter table {table} add column {column} {definition}"
        ))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_servers(app: tauri::AppHandle) -> Result<Vec<Server>, String> {
    let connection = open_database(&app)?;
    let mut statement = connection
        .prepare(
            "select s.id, s.url, count(e.name), s.last_fetched_at
         from servers s left join emojis e on e.host = s.url
         group by s.id order by s.id",
        )
        .map_err(|e| e.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok(Server {
                id: row.get(0)?,
                url: row.get(1)?,
                emoji_count: row.get(2)?,
                last_fetched_at: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

#[derive(Serialize)]
pub struct ServerEmoji {
    pub name: String,
    pub category: Option<String>,
    pub aliases: Vec<String>,
    pub license: Option<String>,
    pub image_url: Option<String>,
}

#[tauri::command]
pub fn list_server_emojis(
    app: tauri::AppHandle,
    host: String,
) -> Result<Vec<ServerEmoji>, String> {
    let connection = open_database(&app)?;
    let mut statement = connection
        .prepare(
            "select name, category, aliases, license, image_url
             from emojis where host = ?1 order by name",
        )
        .map_err(|e| e.to_string())?;
    let rows = statement
        .query_map(params![host], |row| {
            let aliases_json: Option<String> = row.get(2)?;
            let mut aliases = aliases_json
                .and_then(|json| serde_json::from_str::<Vec<String>>(&json).ok())
                .unwrap_or_default();
            aliases.retain(|alias| !alias.trim().is_empty());
            Ok(ServerEmoji {
                name: row.get(0)?,
                category: row.get(1)?,
                aliases,
                license: row.get(3)?,
                image_url: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_server(app: tauri::AppHandle, url: String) -> Result<bool, String> {
    let url = url.trim().trim_end_matches('/').to_string();
    let parsed =
        url::Url::parse(&url).map_err(|_| "有効なサーバーURLを入力してください".to_string())?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err("http または https のサーバーURLを入力してください".to_string());
    }
    open_database(&app)?
        .execute("insert into servers (url) values (?1)", params![url])
        .map(|_| true)
        .map_err(|error| {
            if error.to_string().contains("UNIQUE constraint failed") {
                "このサーバーは登録済みです".to_string()
            } else {
                error.to_string()
            }
        })
}

#[tauri::command]
pub fn remove_server(app: tauri::AppHandle, id: i64) -> Result<bool, String> {
    let connection = open_database(&app)?;
    let url: Option<String> = connection
        .query_row(
            "select url from servers where id = ?1",
            params![id],
            |row| row.get(0),
        )
        .ok();
    if let Some(url) = url {
        connection
            .execute("delete from emojis where host = ?1", params![url])
            .map_err(|e| e.to_string())?;
        connection.execute("delete from hash_parts where hash not in (select hash from emojis where hash is not null)", []).map_err(|e| e.to_string())?;
    }
    connection
        .execute("delete from servers where id = ?1", params![id])
        .map(|_| true)
        .map_err(|e| e.to_string())
}

pub fn server_url(app: &tauri::AppHandle, id: i64) -> Result<String, String> {
    open_database(app)?
        .query_row(
            "select url from servers where id = ?1",
            params![id],
            |row| row.get(0),
        )
        .map_err(|_| "登録済みサーバーが見つかりません".to_string())
}

pub fn replace_server_emojis(
    app: &tauri::AppHandle,
    server_id: i64,
    server_url: &str,
    emojis: &[EmojiRecord],
) -> Result<(), String> {
    let mut connection = open_database(app)?;
    let transaction = connection.transaction().map_err(|e| e.to_string())?;
    transaction
        .execute("delete from emojis where host = ?1", params![server_url])
        .map_err(|e| e.to_string())?;
    transaction.execute("delete from hash_parts where hash not in (select hash from emojis where hash is not null)", []).map_err(|e| e.to_string())?;
    for emoji in emojis {
        let aliases = serde_json::to_string(&emoji.aliases).map_err(|e| e.to_string())?;
        transaction
            .execute(
                "insert or replace into emojis
                 (host, hash, name, category, aliases, license, image_url, etag, last_modified)
                 values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    server_url,
                    &emoji.hash,
                    &emoji.name,
                    &emoji.category,
                    aliases,
                    &emoji.license,
                    &emoji.image_url,
                    &emoji.etag,
                    &emoji.last_modified
                ],
            )
            .map_err(|e| e.to_string())?;
        if let Some(hash) = &emoji.hash {
            for part_bits in [12, 24] {
                for (position, part) in split_hash_parts(hash, part_bits)? {
                    transaction.execute(
                        "insert or ignore into hash_parts (hash, position, part) values (?1, ?2, ?3)",
                        (hash, position, part),
                    ).map_err(|e| e.to_string())?;
                }
            }
        }
    }
    transaction
        .execute(
            "update servers set last_fetched_at = datetime('now') where id = ?1",
            params![server_id],
        )
        .map_err(|e| e.to_string())?;
    transaction.commit().map_err(|e| e.to_string())
}
