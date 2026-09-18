//! 临时诊断：看某个字体文件里的彩色 emoji 被 swash 栅格成了什么。
//! `cargo run --release -p qingjian-render --example emoji_probe -- <字体文件> [字符]`

use cosmic_text::fontdb;
use cosmic_text::{Attrs, Buffer, FontSystem, Metrics, Shaping, SwashCache};

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("用法：emoji_probe <字体文件> [字符]");
    let text = args.next().unwrap_or_else(|| "🙂".to_owned());

    let mut db = fontdb::Database::new();
    db.load_font_file(&path).expect("字体加载失败");
    let families: Vec<String> = db
        .faces()
        .flat_map(|face| face.families.iter().map(|(name, _)| name.clone()))
        .collect();
    println!("字体 {path}\n  字族 {families:?}");

    let mut font_system = FontSystem::new_with_locale_and_db("zh-CN".to_owned(), db);
    let mut cache = SwashCache::new();
    let mut buffer = Buffer::new(&mut font_system, Metrics::new(16.0, 19.0));
    let attrs = Attrs::new().family(cosmic_text::Family::SansSerif);

    for size in [
        7.0_f32, 12.0, 16.0, 20.0, 32.0, 64.0, 96.0, 109.0, 128.0, 137.0, 160.0,
    ] {
        buffer.set_metrics(Metrics::new(size, size * 1.2));
        buffer.set_size(None, None);
        buffer.set_text(&text, &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut font_system, false);

        for run in buffer.layout_runs() {
            for glyph in run.glyphs {
                let physical = glyph.physical((0.0, 0.0), 1.0);
                let report = match cache.get_image(&mut font_system, physical.cache_key) {
                    None => "get_image 返回 None".to_owned(),
                    Some(image) => format!(
                        "{:?} {}×{} 数据 {} 字节",
                        image.content,
                        image.placement.width,
                        image.placement.height,
                        image.data.len()
                    ),
                };
                println!(
                    "字号 {size:>5.1}  字形 {:?}  字体 {:?}  →  {report}",
                    glyph.glyph_id, glyph.font_id
                );
            }
        }
    }
}
