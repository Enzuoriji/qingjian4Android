//! 配置：读进来、改了之后生效、读坏了怎么办。
//!
//! 走的是**真文件**——`Session::open` 收的那个数据目录就是安卓的 `filesDir`，
//! 测试里给它一个临时目录。「用户在设置页里改一项」在这儿就是往 `config.toml` 写一行。

use std::path::{Path, PathBuf};

use super::{
    DENSITY, PORTRAIT_HEIGHT, WIDTH, annotation, config_dir, dictionary, drawn, type_text,
};
use crate::session::Session;
use crate::session::flags;

/// 造一个随包资源目录，里面放几本指定语言的释义表：`[("en", "你好\tint. hello\n"), …]`。
///
/// 与 `super::bundle_with_glossary` 的区别是**语言可以挑几门**——换学习语言那条路
/// 要同时有 en 与 ja 两份才看得出来换没换。
fn bundle_with_glossaries(tag: &str, tables: &[(&str, &str)]) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("qingjian-glossaries-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (language, entries) in tables {
        std::fs::write(dir.join(format!("glossary-{language}.qj")), entries).unwrap();
    }
    dir
}

/// 开一个会话，两块面都画一次（命中矩形才存在）。`data` 里那份 `config.toml` 就是「用户的配置」。
fn open(dictionary: &Path, bundle: Option<&Path>, data: &Path) -> Session {
    let mut session = Session::open(dictionary, "zh-CN", bundle, Some(data)).expect("会话该能打开");
    session.configure(WIDTH, PORTRAIT_HEIGHT, DENSITY, 0.0, false, false);
    session.keyboard_surface();
    session.bar_surface();
    session
}

/// 改配置文件——就是用户在设置页里改了一项之后的样子。
fn write_config(data: &Path, toml: &str) {
    std::fs::write(data.join("config.toml"), toml).unwrap();
}

/// 文件没动时**一次也不该有动作**：壳收到 0 就该什么都不做（重画一张位图是几百微秒，白花）。
#[test]
fn polling_a_quiet_config_does_nothing() {
    let Some(dictionary) = dictionary() else {
        return;
    };
    let data = config_dir("quiet", "[general]\nlearning_language = \"en\"\n");
    let mut session = open(&dictionary, None, &data);

    assert_eq!(session.poll_config(), 0);
    // 打几个字之后再问一遍也一样（打字这条路上不该有任何配置开销）
    type_text(&mut session, "nihao");
    session.bar_surface();
    assert_eq!(session.poll_config(), 0);

    let _ = std::fs::remove_dir_all(&data);
}

/// **改了配置就生效**：用户从设置页回来走的就是这条路——键盘重弹一次，
/// 服务在 `onStartInputView` 里 `poll_config` 一遍。
#[test]
fn changing_the_learning_language_switches_the_glossary() {
    let Some(dictionary) = dictionary() else {
        return;
    };
    let bundle = bundle_with_glossaries(
        "switch_language",
        &[
            ("en", "你好\tint. hello\n"),
            ("ja", "你好\tint. こんにちは\n"),
        ],
    );
    let data = config_dir("switch_language", "[general]\nlearning_language = \"en\"\n");
    let mut session = open(&dictionary, Some(&bundle), &data);

    type_text(&mut session, "nihao");
    assert!(
        annotation(&session).contains("hello"),
        "缺省该是英语那本，实际是 {:?}",
        annotation(&session)
    );

    write_config(&data, "[general]\nlearning_language = \"ja\"\n");
    assert_ne!(session.poll_config(), 0, "配置变了就该有动作");
    assert!(
        annotation(&session).contains("こんにちは"),
        "切到日语后该换成那本的译文，实际是 {:?}",
        annotation(&session)
    );
    // 再问一遍：没变了
    assert_eq!(session.poll_config(), 0);

    let _ = std::fs::remove_dir_all(&bundle);
    let _ = std::fs::remove_dir_all(&data);
}

/// 关掉译文那行，候选条要**矮一截**——高度是会话级的，变了必须重画位图，
/// 不然位图与命中分界（`bar_pixels`）各说各话一拍。
#[test]
fn turning_translations_off_makes_the_bar_shorter() {
    let Some(dictionary) = dictionary() else {
        return;
    };
    let bundle = bundle_with_glossaries("off_bar", &[("en", "你好\tint. hello\n")]);
    let data = config_dir("off_bar", "[general]\nlearning_language = \"en\"\n");
    let mut session = open(&dictionary, Some(&bundle), &data);

    type_text(&mut session, "nihao");
    let tall = session.bar_height();

    write_config(&data, "[general]\nlearning_language = \"off\"\n");
    let mask = session.poll_config();

    assert_ne!(mask & flags::BAR, 0, "高度变了，掩码里该有 BAR");
    assert!(
        session.bar_height() < tall,
        "关掉译文后该矮一截：{} vs {tall}",
        session.bar_height()
    );
    assert!(annotation(&session).is_empty(), "关了就不该有译文");

    let _ = std::fs::remove_dir_all(&bundle);
    let _ = std::fs::remove_dir_all(&data);
}

/// 配置文件被改坏了：**当没变**——不能把手上这份能用的弄丢，也不能反复去啃同一个坏文件。
#[test]
fn a_broken_config_keeps_the_last_good_one() {
    let Some(dictionary) = dictionary() else {
        return;
    };
    let data = config_dir("broken_stays", "[fuzzy]\nz_zh = true\n");
    let mut session = open(&dictionary, None, &data);

    write_config(&data, "[fuzzy\nz_zh = false\n");
    assert_eq!(session.poll_config(), 0, "读不出来就当没变");
    assert!(session.config.error().is_some(), "该把原因记下来给排查用");

    // 开着的那条模糊音还在——旧配置没被弄丢
    type_text(&mut session, "zongguo");
    assert!(
        drawn(&session).contains(&"中国"),
        "旧配置该还在生效，实际是 {:?}",
        drawn(&session)
    );
    // 同一份坏文件不反复重试（原文没变就直接返回）
    assert_eq!(session.poll_config(), 0);

    // 改好了就正常生效——「原文变了才重试」没把路堵死
    write_config(&data, "[fuzzy]\nz_zh = false\n");
    let _ = session.poll_config();
    assert!(session.config.error().is_none(), "改好之后不该还记着旧错误");

    let _ = std::fs::remove_dir_all(&data);
}

/// 模糊音是南方口音的刚需（z / zh、n / l 不分）。这条守着配置文件里那九个勾选框
/// **真的推到了引擎上**——启动读一次、改了之后热重载一次，两条路都走。
///
/// 断言的是 `Engine::fuzzy()`（引擎自己那份状态），不是「某个词出不出来」：
/// 挑词当靶子靠不住——不开模糊音时 `zongguo` 也能出「中国」，引擎另有一条容错路径。
/// 而「模糊音有没有生效」本来就是 core 那边的测试管的事。
#[test]
fn fuzzy_rules_from_the_config_reach_the_engine() {
    let Some(dictionary) = dictionary() else {
        return;
    };

    let data = config_dir("fuzzy", "[fuzzy]\nz_zh = false\n");
    let mut session = open(&dictionary, None, &data);
    assert!(!session.engine.fuzzy().any(), "缺省一条都不该开");

    // 改配置 → 热重载 → 引擎跟着变
    write_config(&data, "[fuzzy]\nz_zh = true\nan_ang = true\n");
    assert_ne!(session.poll_config(), 0, "配置变了就该有动作");
    assert!(session.engine.fuzzy().z_zh, "热重载该把 z_zh 推给引擎");
    assert!(session.engine.fuzzy().an_ang);
    assert!(!session.engine.fuzzy().n_l, "没开的那条不该跟着一起开");

    // 关回去也生效
    write_config(&data, "[fuzzy]\nz_zh = false\n");
    let _ = session.poll_config();
    assert!(!session.engine.fuzzy().any());

    // 启动时读的那一份同样算数
    write_config(&data, "[fuzzy]\nn_l = true\n");
    let fresh = open(&dictionary, None, &data);
    assert!(fresh.engine.fuzzy().n_l, "开会话时该按文件里那份设上");

    let _ = std::fs::remove_dir_all(&data);
}

/// 工具页那一格「设置」点了之后：会话记一笔账，掩码里带上 `SETTINGS`，取走就清掉。
///
/// 真正 `startActivity` 是壳的事（会话碰不到安卓的窗口系统），这里只守着**这笔账**。
#[test]
fn tapping_the_settings_cell_asks_the_shell_to_open_the_page() {
    let Some(dictionary) = dictionary() else {
        return;
    };
    let data = config_dir("open_settings", "");
    let mut session = open(&dictionary, None, &data);

    assert!(!session.take_settings(), "还没人点过");
    session.apply(crate::action::Act::OpenSettings);
    assert_ne!(session.mask() & flags::SETTINGS, 0, "掩码里该带上这一笔");
    assert!(session.take_settings(), "取了就该有");
    assert!(!session.take_settings(), "取过就没了");
    assert_eq!(session.mask() & flags::SETTINGS, 0);

    let _ = std::fs::remove_dir_all(&data);
}
