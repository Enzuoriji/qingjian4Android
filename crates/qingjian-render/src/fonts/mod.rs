//! 字体库：不扫系统字体目录（fontdb 全扫几百毫秒、几十 MB），按平台清单只加载界面字体、中文、日文、emoji 几个文件。
//!
//! 文件走 fontdb 的 mmap 加载，只解析名字表与 cmap，Apple Color Emoji 那种 190 MB 的文件也只在用到字形时才读页。
//! 中日同形字按 locale 回退（cosmic-text 的平台回退表：zh-CN → PingFang SC，ja → Hiragino Sans）。

#[cfg(target_os = "android")]
mod android;
#[cfg(target_os = "windows")]
pub mod directwrite;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
mod trak;
mod ui_font;
#[cfg(target_os = "windows")]
mod windows;

use std::path::{Path, PathBuf};

use cosmic_text::FontSystem;
use cosmic_text::fontdb::{Database, Family};

use crate::error::RenderError;

pub(crate) use trak::Trak;
pub use ui_font::UiFont;

#[cfg(target_os = "windows")]
use self::windows as platform;
#[cfg(target_os = "android")]
use android as platform;
#[cfg(target_os = "linux")]
use linux as platform;
#[cfg(target_os = "macos")]
use macos as platform;

/// 交给 cosmic-text 的回退表。安卓自带一份：cosmic-text 在该平台上给的是空表，
/// 中日同形字会落到 `NotoSansCJK-Regular.ttc` 的第一个面（日文字形）。见 `android.rs`。
///
/// 用 `use` 而不是 `type`：构造器的参数收的是 `impl Fallback`（值），
/// 单元结构体的 `use` 别名会把类型与构造函数一起带进来，`type` 别名则只有类型。
#[cfg(target_os = "android")]
use android::AndroidFallback as PlatformFallback;
#[cfg(not(target_os = "android"))]
use cosmic_text::PlatformFallback;

pub struct FontLibrary {
    /// 已加载的字体。
    db: Database,

    /// 界面字体的字族名（`SansSerif` 映射到它）。
    ui_family: String,

    /// 中日同形字回退用的 locale，如 `zh-CN`。
    locale: String,
}

impl FontLibrary {
    /// 按平台清单加载系统字体。`locale` 决定 Han 字形选哪家（`zh-CN` / `zh-TW` / `ja`）。
    pub fn system(locale: &str) -> Result<Self, RenderError> {
        Self::build(locale, None, None)
    }

    /// 同 [`Self::system`]，但 emoji 字体用随包带的这几张（非空时**顶替**系统里那几张，不是加在后面）。
    ///
    /// 要顶替而不是追加，是因为两边字族同名（都叫 `Noto Color Emoji`），都在库里的话按哪张由
    /// 查表顺序定，说不清。安卓 15 起的系统字体是纯 COLR v1（v0 记录 0 条），swash 读不到图层，
    /// emoji 会画成空白；随包的旧版是 CBDT 位图，swash 走得通。见 `assets/emoji/README.md`。
    pub fn system_with_emoji_fonts(locale: &str, emoji: &[PathBuf]) -> Result<Self, RenderError> {
        Self::build(locale, None, Some(emoji))
    }

    /// 界面字体换成用户指定的字族，系统字体仍加载在后面当回退；指定的字体一个文件都没加载到或名字对不上就退回系统字体。
    pub fn with_ui_font(locale: &str, ui_font: &UiFont) -> Result<Self, RenderError> {
        Self::build(locale, Some(ui_font), None)
    }

    fn build(
        locale: &str,
        ui_font: Option<&UiFont>,
        emoji: Option<&[PathBuf]>,
    ) -> Result<Self, RenderError> {
        let mut db = Database::new();
        let custom_family = ui_font.and_then(|font| {
            let loaded = font.files.iter().filter(|path| load(&mut db, path)).count();
            let found = db.faces().any(|face| {
                face.families
                    .iter()
                    .any(|(name, _)| name.eq_ignore_ascii_case(&font.family))
            });
            if loaded > 0 && found {
                Some(font.family.clone())
            } else {
                tracing::warn!(
                    family = font.family,
                    files = font.files.len(),
                    loaded,
                    "指定的候选窗字体没找到，用系统字体"
                );
                None
            }
        });
        let ui =
            load_first(&mut db, &platform::ui_fonts()).ok_or_else(|| RenderError::NoUiFont {
                tried: platform::ui_fonts(),
            })?;
        let ui_family = custom_family.unwrap_or_else(|| {
            db.face(ui)
                .and_then(|face| face.families.first().map(|(name, _)| name.clone()))
                .unwrap_or_else(|| "sans-serif".to_owned())
        });
        db.set_sans_serif_family(ui_family.clone());
        let emoji_fonts = emoji.map_or_else(platform::emoji_fonts, <[PathBuf]>::to_vec);
        for path in platform::script_fonts(locale)
            .into_iter()
            .chain(emoji_fonts)
        {
            if !load(&mut db, &path) {
                tracing::debug!(path = %path.display(), "字体文件不存在，跳过");
            }
        }
        tracing::debug!(faces = db.len(), ui_family, "字体库就绪");
        Ok(Self {
            db,
            ui_family,
            locale: locale.to_owned(),
        })
    }

    /// 已加载的字族名，按加载顺序去重。
    pub fn families(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for face in self.db.faces() {
            for (name, _) in &face.families {
                if !names.contains(name) {
                    names.push(name.clone());
                }
            }
        }
        names
    }

    pub fn ui_family(&self) -> &str {
        &self.ui_family
    }

    /// 交给 cosmic-text。
    ///
    /// 走的 `_and_fallback` 那个构造器，回退表由 [`PlatformFallback`] 按平台挑；
    /// `new_with_locale_and_db` 内部也是转发给它、只是永远用 cosmic-text 的平台表。
    pub(crate) fn into_font_system(self) -> FontSystem {
        let mut db = self.db;
        db.set_sans_serif_family(self.ui_family);
        FontSystem::new_with_locale_and_db_and_fallback(self.locale, db, PlatformFallback)
    }
}

/// 界面字体用的字族。
pub(crate) const UI_FAMILY: Family<'static> = Family::SansSerif;

/// 依次尝试，加载成功的第一个文件的第一张面。
fn load_first(db: &mut Database, paths: &[PathBuf]) -> Option<cosmic_text::fontdb::ID> {
    paths.iter().find_map(|path| {
        let before: Vec<_> = db.faces().map(|f| f.id).collect();
        load(db, path)
            .then(|| db.faces().map(|f| f.id).find(|id| !before.contains(id)))
            .flatten()
    })
}

/// 文件存在且能解析就加载；返回是否加载了。
fn load(db: &mut Database, path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    match db.load_font_file(path) {
        Ok(()) => true,
        Err(error) => {
            tracing::warn!(path = %path.display(), %error, "字体文件解析失败");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use cosmic_text::fontdb::Database;
    use cosmic_text::{Attrs, Buffer, FontSystem, Metrics, Shaping, SwashCache, SwashContent};

    /// 随包的 emoji 字体。`assets/emoji/README.md` 写了为什么要带它。
    fn bundled_emoji() -> Option<PathBuf> {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/emoji/NotoColorEmoji.ttf");
        path.is_file().then_some(path)
    }

    /// 随包那张 emoji 字体必须能栅格成**彩色**位图。
    ///
    /// 直接问 swash 而不是走 `Renderer`：`Renderer` 挑字体靠回退表，而回退表是分平台的
    /// （Windows 表里写的是 `Segoe UI Emoji`），拿它测会变成「测平台回退表」。
    /// 这里把字族直接指到随包这张，测的才是这张字体本身画不画得出来。
    #[test]
    fn bundled_emoji_font_rasterizes_in_color() {
        let Some(path) = bundled_emoji() else {
            return;
        };
        let mut db = Database::new();
        db.load_font_file(&path).expect("随包的 emoji 字体加载失败");
        db.set_sans_serif_family("Noto Color Emoji");

        let mut font_system = FontSystem::new_with_locale_and_db("zh-CN".to_owned(), db);
        let mut cache = SwashCache::new();
        let mut buffer = Buffer::new(&mut font_system, Metrics::new(16.0, 19.0));
        buffer.set_size(None, None);
        buffer.set_text("🙂", &Attrs::new(), Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut font_system, false);

        let glyph = buffer
            .layout_runs()
            .find_map(|run| run.glyphs.first())
            .expect("没整形出任何字形");
        let physical = glyph.physical((0.0, 0.0), 1.0);
        let Some(image) = cache.get_image(&mut font_system, physical.cache_key) else {
            panic!("整形出字形就该能栅格");
        };
        assert!(
            matches!(image.content, SwashContent::Color),
            "该是彩色位图，实际 {:?}——换成 COLR v1 的字体就会这样",
            image.content
        );
        assert!(
            image.placement.width > 0 && image.placement.height > 0,
            "位图不能是 0×0 的空图"
        );
    }
}
