use crate::{backend::EmojiMatch, common::Ctx};
use std::rc::Rc;

/// 読み込んだ画像に、絵文字名・タグ・ライセンスなどの編集内容と照合候補が付与されたもの。
#[derive(Clone, PartialEq)]
pub struct EditingEmoji {
    pub file_name: String,
    pub preview: String,
    pub bytes: Rc<Vec<u8>>,
    pub md5: String,
    pub hash: Option<String>,
    pub matches: Vec<EmojiMatch>,
    pub error: Option<String>,
    pub emoji_name: String,
    pub category: String,
    pub aliases: Vec<String>,
    pub license: String,
    pub selected: bool,
}

pub enum EditingEmojisTag {}

/// 編集中の絵文字リストを MainTab 配下で共有するコンテキスト。
pub type EditingEmojis = Ctx<Vec<EditingEmoji>, EditingEmojisTag>;

pub trait EditingEmojisExt {
    fn get_by_md5(&self, md5_hash: &str) -> Option<&EditingEmoji>;
    fn get_by_md5_mut(&mut self, md5_hash: &str) -> Option<&mut EditingEmoji>;
}

impl EditingEmojisExt for Vec<EditingEmoji> {
    fn get_by_md5(&self, md5_hash: &str) -> Option<&EditingEmoji> {
        self.iter().find(|emoji| emoji.md5 == md5_hash)
    }

    fn get_by_md5_mut(&mut self, md5_hash: &str) -> Option<&mut EditingEmoji> {
        self.iter_mut().find(|emoji| emoji.md5 == md5_hash)
    }
}
