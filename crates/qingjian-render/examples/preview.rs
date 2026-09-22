//! 离线预览：`cargo run --release -p qingjian-render --example preview -- --out target/render-preview`
//! 把样例帧按浅 / 深色、竖 / 横排画成 PNG，与各平台原生候选窗截图并排比；`--measure` 只量几段文字的宽度与原生对数；
//! 末尾列出验收行每个字形落到了哪家字体。不是日常工具，改渲染器时拿来核对。

use std::path::PathBuf;
use std::time::Instant;

use clap::Parser;
use qingjian_render::{
    FontLibrary, Frame, Key, KeyId, KeyboardLayout, KeyboardState, KeyboardTheme, Layout, Popup,
    Preedit, PreeditSegment, PreeditStyle, Renderer, Row, Shadow, ShiftState, StatusCell, Theme,
    Tone,
};

/// 预览键盘用的宽度（点）——按一台常见手机的竖屏宽。
const KEYBOARD_WIDTH: f32 = 360.0;

/// 预览候选条用的宽度（点），同上。
const BAR_WIDTH: f32 = 360.0;

#[derive(Parser)]
struct Args {
    /// PNG 输出目录。
    #[arg(long, default_value = "target/render-preview")]
    out: PathBuf,

    /// 点 → 像素倍数（Retina 为 2）。
    #[arg(long, default_value_t = 2.0)]
    scale: f32,

    /// 中日字形回退用的 locale。
    #[arg(long, default_value = "zh-CN")]
    locale: String,

    /// 不画阴影（对照壳自己带系统阴影的截图时用）。
    #[arg(long)]
    no_shadow: bool,

    /// 只量几段文字的宽度（点），不出图；与 AppKit 的 NSAttributedString.size() 对数。
    #[arg(long)]
    measure: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "qingjian_render=debug".into()),
        )
        .init();
    let args = Args::parse();
    std::fs::create_dir_all(&args.out)?;

    let started = Instant::now();
    let library = FontLibrary::system(&args.locale)?;
    println!(
        "字体库：{:?}，界面字体 {}",
        started.elapsed(),
        library.ui_family()
    );
    println!("已加载字族：{}", library.families().join(" / "));
    let mut renderer = Renderer::new(library);
    if args.measure {
        for text in [
            "int. hello · int. hi",
            "hello",
            "ni'hao",
            "1/6",
            "你好",
            "phr. you change",
            "int. ",
            "·",
            " · ",
            "hi",
            "你好像",
            "開発する",
        ] {
            let widths: Vec<String> = [11.0, 12.0, 16.0]
                .into_iter()
                .map(|size| format!("{size}pt={:.2}", renderer.measure_points(text, size)))
                .collect();
            println!("{text:<24} {}", widths.join("  "));
        }
        return Ok(());
    }
    let shadow = (!args.no_shadow).then_some(Shadow::mac_panel());

    let scenes: [(&str, Frame, Layout); 6] = [
        ("nihao-vertical", nihao(), Layout::Vertical),
        (
            "nihao-horizontal",
            nihao_with_sentence(),
            Layout::Horizontal,
        ),
        ("cloud-vertical", cloud(), Layout::Vertical),
        ("corrected-vertical", corrected_japanese(), Layout::Vertical),
        (
            "corrected-horizontal",
            corrected_japanese(),
            Layout::Horizontal,
        ),
        ("probe", probe(), Layout::Vertical),
    ];
    for (theme_name, theme) in [("light", Theme::light()), ("dark", Theme::dark())] {
        for (scene, frame, layout) in &scenes {
            let started = Instant::now();
            let rendered = renderer.render(frame, *layout, &theme, args.scale, shadow.as_ref())?;
            let elapsed = started.elapsed();
            let path = args.out.join(format!("{scene}-{theme_name}.png"));
            rendered.pixmap.save_png(&path)?;
            let (w, h) = rendered.content_size_points();
            println!(
                "{:<28} {:>4.0}×{:<4.0}pt  {:>8.2?}  {}",
                format!("{scene}-{theme_name}"),
                w,
                h,
                elapsed,
                path.display()
            );
        }
    }

    // Windows 的悬浮状态条：三格
    let cells = [
        StatusCell::text("中 · 小鹤", true),
        StatusCell::text("，。", true),
        StatusCell::Gear,
    ];
    for (theme_name, theme) in [("light", Theme::light()), ("dark", Theme::dark())] {
        let status = renderer.render_status(&cells, &theme, args.scale, shadow.as_ref())?;
        let path = args.out.join(format!("status-{theme_name}.png"));
        status.rendered.pixmap.save_png(&path)?;
        let (w, h) = status.rendered.content_size_points();
        println!(
            "{:<28} {:>4.0}×{:<4.0}pt  格边界 {:?}  {}",
            format!("status-{theme_name}"),
            w,
            h,
            status.cell_edges,
            path.display()
        );
    }

    // 键预览气泡：字母键一个、窄键（句号）一个、图标键（退格）一个
    for (theme_name, keyboard_theme) in [
        ("light", KeyboardTheme::light()),
        ("dark", KeyboardTheme::dark()),
    ] {
        for (name, key, content) in [
            ("letter", Key::letter('A', '1'), None),
            ("narrow", Key::new(KeyId::Period, 1.0), None),
            ("icon", Key::new(KeyId::Backspace, 1.5), None),
            // ⌫ 往上滑之后气泡改说的那句话
            ("clear", Key::new(KeyId::Backspace, 1.5), Some("松手清空")),
        ] {
            let started = Instant::now();
            let popup = match content {
                Some(text) => Popup::Text(text),
                None => Popup::Key(&key),
            };
            let rendered = renderer.render_key_popup(
                popup,
                &KeyboardState::default(),
                29.7,
                42.25,
                &keyboard_theme,
                args.scale,
            )?;
            let elapsed = started.elapsed();
            let path = args.out.join(format!("popup-{name}-{theme_name}.png"));
            rendered.pixmap.save_png(&path)?;
            let (w, h) = rendered.content_size_points();
            println!(
                "{:<28} {:>4.0}×{:<4.0}pt  {:>8.2?}  {}",
                format!("popup-{name}-{theme_name}"),
                w,
                h,
                elapsed,
                path.display()
            );
        }
    }

    // 软键盘：常态与「Shift 锁定 + 正按着 A」两种状态，明暗两套主题
    let keyboard_layout = KeyboardLayout::letters();
    for (theme_name, keyboard_theme) in [
        ("light", KeyboardTheme::light()),
        ("dark", KeyboardTheme::dark()),
    ] {
        for (state_name, state) in [
            ("idle", KeyboardState::default()),
            (
                "shift-a",
                KeyboardState {
                    shift: ShiftState::Locked,
                    pressed: Some(KeyId::Letter('A')),
                    ..KeyboardState::default()
                },
            ),
        ] {
            let started = Instant::now();
            let rendered = renderer.render_keyboard(
                &keyboard_layout,
                &state,
                KEYBOARD_WIDTH,
                0.0,
                &keyboard_theme,
                args.scale,
            )?;
            let elapsed = started.elapsed();
            let path = args
                .out
                .join(format!("keyboard-{state_name}-{theme_name}.png"));
            rendered.rendered.pixmap.save_png(&path)?;
            let (w, h) = rendered.rendered.content_size_points();
            println!(
                "{:<28} {:>4.0}×{:<4.0}pt  {:>8.2?}  {} 个键  {}",
                format!("keyboard-{state_name}-{theme_name}"),
                w,
                h,
                elapsed,
                rendered.keys.len(),
                path.display()
            );
        }
    }

    // 候选条（安卓）：空态与一页候选。宽度按手机竖屏，高度是主题定死的
    for (theme_name, theme) in [("light", Theme::light()), ("dark", Theme::dark())] {
        for (state_name, bar_frame) in [
            ("empty", Frame::default()),
            ("page", bar_page()),
            ("sentence", bar_with_sentence()),
        ] {
            let started = Instant::now();
            let rendered =
                renderer.render_bar(&bar_frame, BAR_WIDTH, &theme, args.scale, 0.0, true)?;
            let elapsed = started.elapsed();
            let path = args.out.join(format!("bar-{state_name}-{theme_name}.png"));
            rendered.rendered.pixmap.save_png(&path)?;
            let (w, h) = rendered.rendered.content_size_points();
            println!(
                "{:<28} {:>4.0}×{:<4.0}pt  {:>8.2?}  {} 个可点区域  {}",
                format!("bar-{state_name}-{theme_name}"),
                w,
                h,
                elapsed,
                rendered.hits.len(),
                path.display()
            );
        }
    }
    println!(
        "候选条高度：组句 {:.1}pt、没组句 {:.1}pt（只在「在不在组句」之间变，与候选多少无关）",
        Renderer::bar_height(&Theme::light(), true, true),
        Renderer::bar_height(&Theme::light(), false, false),
    );

    for probe in [
        "青简 hello 🙂 日本語 骨直曜",
        "開発(かいはつ)する",
        "int. hello · int. hi",
    ] {
        println!(
            "「{probe}」各字形字体：{}",
            renderer.trace_families(probe, &Theme::light()).join(" → ")
        );
    }
    Ok(())
}

// 样例帧：与真机上敲同样拼音看到的候选窗对照，所以内容要和引擎当时给的一致（人工从截图抄）。

/// 真机敲「nihao」看到的第一页（2026-09-13 从截图抄），拼音行带光标、译文、生词橙色、页码。
fn nihao() -> Frame {
    Frame {
        preedit: Some(Preedit::plain("ni'hao", 6)),
        rows: vec![
            annotated(
                0,
                "你好",
                &[
                    ("int. ", Tone::Faint),
                    ("hello", Tone::Gloss),
                    (" · ", Tone::Faint),
                    ("int. ", Tone::Faint),
                    ("hi", Tone::Gloss),
                ],
                false,
            ),
            annotated(1, "👋", &[("你好", Tone::Gloss)], false),
            annotated(2, "你好好", &[], false),
            annotated(
                3,
                "你好像",
                &[("phr. ", Tone::Faint), ("you seem", Tone::Fresh)],
                false,
            ),
            annotated(4, "你好久", &[], false),
            annotated(5, "你好看", &[], false),
            annotated(6, "你哈", &[], false),
            annotated(
                7,
                "你换",
                &[
                    ("phr. ", Tone::Faint),
                    ("you change", Tone::Fresh),
                    (" · ", Tone::Faint),
                    ("phr. ", Tone::Faint),
                    ("you swap", Tone::Fresh),
                ],
                false,
            ),
            annotated(
                8,
                "你会",
                &[("phr. ", Tone::Faint), ("you will", Tone::Fresh)],
                false,
            ),
        ],
        highlighted: Some(0),
        footer: Some("1/6".to_owned()),
        sentence: None,
        status: None,
    }
}

/// 候选条上的一页：手机一页 5 个（比桌面的 9 个少，也比一开始定的 6 个少——
/// 360pt 宽下 6 格放不下三字词，会截成「你…」），取 nihao 那页的前 5 条。
fn bar_page() -> Frame {
    let mut frame = nihao();
    frame.rows.truncate(5);
    frame.footer = Some("1/2".to_owned());
    frame
}

/// 候选条 + 云联想给的整句补全：最下面那行**左边译文、右边青色的整句**（安卓的排版）。
fn bar_with_sentence() -> Frame {
    let mut frame = bar_page();
    frame.sentence = Some("你好，很高兴认识你！".to_owned());
    frame
}

/// 横排真机截图那一次云端整句到了：拼音行右侧带云朵的整句补全。
fn nihao_with_sentence() -> Frame {
    let mut frame = nihao();
    frame.sentence = Some("你好，很高兴认识你！".to_owned());
    frame
}

/// 带云联想的一页：整句补全与云端词各带云朵。
fn cloud() -> Frame {
    let mut frame = nihao();
    frame.rows.truncate(3);
    frame
        .rows
        .push(annotated(3, "你好吗", &[("hello?", Tone::Gloss)], true));
    frame.sentence = Some("你好，世界".to_owned());
    frame.footer = Some("1/3".to_owned());
    frame
}

/// 纠错后的拼音行（删除线 + 淡色剩余）加日文译词（汉字注假名）。
fn corrected_japanese() -> Frame {
    Frame {
        preedit: Some(Preedit {
            segments: vec![
                PreeditSegment {
                    text: "kai".to_owned(),
                    style: PreeditStyle::Typed,
                },
                PreeditSegment {
                    text: "fs".to_owned(),
                    style: PreeditStyle::Struck,
                },
                PreeditSegment {
                    text: "'fa".to_owned(),
                    style: PreeditStyle::Rest,
                },
            ],
            cursor: 5,
        }),
        rows: vec![
            annotated(
                0,
                "开发",
                &[
                    ("v. ", Tone::Faint),
                    ("開発", Tone::Gloss),
                    ("(かいはつ)", Tone::Faint),
                    ("する", Tone::Gloss),
                ],
                false,
            ),
            annotated(
                1,
                "开",
                &[
                    ("v. ", Tone::Faint),
                    ("開", Tone::Fresh),
                    ("(ひら)", Tone::Faint),
                    ("く", Tone::Fresh),
                ],
                false,
            ),
        ],
        highlighted: Some(1),
        footer: None,
        sentence: None,
        status: Some("已删除「开放」".to_owned()),
    }
}

/// 四条验收用的一行：汉字（zh 字形）、英文、彩色 emoji、日文假名与汉字。
fn probe() -> Frame {
    Frame {
        preedit: None,
        rows: vec![Row::plain(0, "青简 hello 🙂 日本語 骨直曜")],
        highlighted: None,
        footer: None,
        sentence: None,
        status: None,
    }
}

fn annotated(index: usize, text: &str, annotation: &[(&str, Tone)], cloud: bool) -> Row {
    Row {
        index: (index + 1).to_string(),
        text: text.to_owned(),
        annotation: annotation
            .iter()
            .map(|(s, tone)| ((*s).to_owned(), *tone))
            .collect(),
        cloud,
    }
}
