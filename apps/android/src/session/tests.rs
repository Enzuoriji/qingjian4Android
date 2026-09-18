//! 会话的端到端测试：**在电脑上跑，不需要模拟器或真机**。
//!
//! 喂的是真实坐标，走的是完整触摸链路——点位从渲染器真正画出来的命中矩形里取，
//! 所以布局一改、命中算错，这里先炸。「能打字能选词」这条验收因此是可断言的。

use super::Session;
use crate::action::{Act, Command};
use crate::touch::MotionAction;
use qingjian_core::CandidateKind;
use qingjian_render::{BarHitId, KeyId, Panel};
use std::path::PathBuf;

/// 单指测试用的 pointer id。多点触控的用例自己给别的编号。
const POINTER: i32 = 0;

/// 验收用的屏幕宽（点）与密度，按一台常见手机竖屏。
const WIDTH: f32 = 360.0;
const DENSITY: f32 = 2.75;

/// 词库：产品数据优先，退回基础词库（9.3 万条，翻页这些才验得出来），最后是随包样例。
/// 都没有就跳过（CI 容器里可能没有产品数据）。
fn dictionary() -> Option<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    [
        "data/generated/dict.qj",
        "data/generated/dict.tsv",
        "assets/lexicon/dict.tsv",
        "assets/sample/dict.tsv",
    ]
    .into_iter()
    .map(|rel| root.join(rel))
    .find(|path| path.is_file())
}

/// 某个键中心的坐标（**整块输入视图的像素**，与壳传进来的一致）。
fn key_centre(session: &Session, id: KeyId) -> (f32, f32) {
    let (x, y, width, height) = key_rect(session, id);
    (x + width / 2.0, y + height / 2.0)
}

/// 候选条上某一块的中心。候选条就在视图顶部，所以 y 不用再加偏移。
fn bar_centre(session: &Session, id: BarHitId) -> (f32, f32) {
    let bar = session.bar.as_ref().expect("候选条还没画过，没有命中矩形");
    let hit = bar
        .hits
        .iter()
        .find(|hit| hit.id == id)
        .unwrap_or_else(|| panic!("候选条上没有 {id:?}"));
    (hit.x + hit.width / 2.0, hit.y + hit.height / 2.0)
}

/// 在 `(x, y)` 上按下再抬起。位图取一次，命中矩形跟上最新的候选（与壳的行为一致）。
fn tap_at(session: &mut Session, x: f32, y: f32) {
    session.touch(MotionAction::Down, POINTER, x, y);
    session.touch(MotionAction::Up, POINTER, x, y);
    session.bar_surface();
    session.keyboard_surface();
}

fn tap_key(session: &mut Session, id: KeyId) {
    let (x, y) = key_centre(session, id);
    tap_at(session, x, y);
}

fn tap_bar(session: &mut Session, id: BarHitId) {
    let (x, y) = bar_centre(session, id);
    tap_at(session, x, y);
}

fn type_text(session: &mut Session, text: &str) {
    for letter in text.chars() {
        tap_key(session, KeyId::Letter(letter));
    }
}

fn ready() -> Option<Session> {
    let mut session = Session::open(&dictionary()?, "zh-CN", None).ok()?;
    session.configure(WIDTH, DENSITY, 0.0, false);
    // 两块面都画一次，命中矩形才存在
    session.keyboard_surface();
    session.bar_surface();
    Some(session)
}

/// 候选条此刻画出来的词。
fn drawn(session: &Session) -> Vec<&str> {
    session
        .frame
        .rows
        .iter()
        .map(|row| row.text.as_str())
        .collect()
}

/// 拼音行此刻的内容。
fn preedit(session: &Session) -> Option<String> {
    session.frame.preedit.as_ref().map(|p| p.text())
}

#[test]
fn tapping_nihao_offers_it_in_the_bar() {
    let Some(mut session) = ready() else {
        return;
    };
    type_text(&mut session, "nihao");

    assert_eq!(
        preedit(&session).as_deref(),
        Some("ni'hao"),
        "拼音行该把音节用 ' 切开"
    );
    assert!(
        drawn(&session).contains(&"你好"),
        "候选条里该有「你好」，实际画的是 {:?}",
        drawn(&session)
    );
    assert!(
        drawn(&session).len() <= 5,
        "一页最多 5 个，实际 {:?}",
        drawn(&session)
    );
    assert_eq!(session.frame.highlighted, Some(0), "默认高亮第一个");
    assert_eq!(session.frame.rows[0].index, "1", "序号从 1 起");
}

#[test]
fn tapping_a_candidate_commits_it_and_clears_the_pinyin() {
    let Some(mut session) = ready() else {
        return;
    };
    type_text(&mut session, "nihao");
    // 「你好」排第几由排序决定，所以先在画出来的那几个里找到它
    let position = drawn(&session)
        .iter()
        .position(|text| *text == "你好")
        .expect("候选里该有「你好」");

    tap_bar(&mut session, BarHitId::Candidate(position));

    assert_eq!(
        session.take_commit().as_deref(),
        Some("你好"),
        "点候选该把词上屏"
    );
    assert!(
        session.frame.preedit.is_none(),
        "上屏之后拼音该清空，实际还剩 {:?}",
        preedit(&session)
    );
    assert!(drawn(&session).is_empty(), "拼音没了就不该还有候选");
    assert_eq!(
        session.take_preedit(),
        "",
        "拼音行该镜像成空串，壳据此结束组字"
    );
}

#[test]
fn space_commits_the_highlighted_candidate() {
    let Some(mut session) = ready() else {
        return;
    };
    type_text(&mut session, "nihao");
    let first = drawn(&session).first().map(|text| (*text).to_owned());

    tap_key(&mut session, KeyId::Space);

    assert_eq!(session.take_commit(), first, "空格该上屏高亮那个");
    assert!(drawn(&session).is_empty());
}

#[test]
fn space_without_candidates_is_a_space() {
    let Some(mut session) = ready() else {
        return;
    };
    tap_key(&mut session, KeyId::Space);
    assert_eq!(session.take_commit().as_deref(), Some(" "));
}

#[test]
fn enter_gives_back_the_pinyin_itself() {
    let Some(mut session) = ready() else {
        return;
    };
    type_text(&mut session, "nihao");

    tap_key(&mut session, KeyId::Enter);

    assert_eq!(
        session.take_commit().as_deref(),
        Some("nihao"),
        "回车该把拼音原文上屏"
    );
    assert!(drawn(&session).is_empty());
    assert!(session.take_commands().is_empty(), "有拼音时不该交给应用");
}

#[test]
fn enter_without_pinyin_goes_to_the_app() {
    let Some(mut session) = ready() else {
        return;
    };
    tap_key(&mut session, KeyId::Enter);
    assert_eq!(session.take_commands(), vec![Command::Enter.code()]);
}

#[test]
fn backspace_deletes_one_letter_and_then_goes_to_the_app() {
    let Some(mut session) = ready() else {
        return;
    };
    // niha → ni'ha（三个字母：n i h，a 还没成音节）
    type_text(&mut session, "niha");
    assert_eq!(preedit(&session).as_deref(), Some("ni'ha"));

    tap_key(&mut session, KeyId::Backspace);

    assert_eq!(
        preedit(&session).as_deref(),
        Some("ni'h"),
        "退格该删掉一个字母"
    );
    assert!(session.take_commands().is_empty(), "还有拼音时不该交给应用");

    // 剩下的三个也删掉：这几下都该被引擎吃掉
    for _ in 0..3 {
        tap_key(&mut session, KeyId::Backspace);
    }
    assert!(session.frame.preedit.is_none(), "该删空了");
    assert!(
        session.take_commands().is_empty(),
        "删空拼音那几下不该惊动应用"
    );

    // 拼音已经空了，这一下才该交给应用去删字符
    tap_key(&mut session, KeyId::Backspace);
    assert_eq!(session.take_commands(), vec![Command::Backspace.code()]);
}

#[test]
fn the_clear_button_empties_the_buffer() {
    let Some(mut session) = ready() else {
        return;
    };
    type_text(&mut session, "nihao");

    tap_bar(&mut session, BarHitId::Clear);

    assert!(session.frame.preedit.is_none(), "清空该把拼音清掉");
    assert!(drawn(&session).is_empty());
    assert_eq!(session.take_commit(), None, "清空不上屏任何东西");
}

#[test]
fn paging_shows_the_next_batch() {
    let Some(mut session) = ready() else {
        return;
    };
    // 「shi」在基础词库里有一大把候选，够翻页
    type_text(&mut session, "shi");
    let footer = session.frame.footer.clone().expect("多于十页该有页码");
    assert!(footer.starts_with("1/"), "页码该从第一页起，实际 {footer}");
    let first_page: Vec<String> = drawn(&session).iter().map(|s| (*s).to_owned()).collect();

    tap_bar(&mut session, BarHitId::PageNext);

    assert!(
        session.frame.footer.as_deref().unwrap().starts_with("2/"),
        "该翻到第二页，实际 {:?}",
        session.frame.footer
    );
    let second_page: Vec<String> = drawn(&session).iter().map(|s| (*s).to_owned()).collect();
    assert_ne!(first_page, second_page, "第二页该是别的候选");
    assert_eq!(second_page.len(), 5, "满页该有 5 个");

    tap_bar(&mut session, BarHitId::PagePrev);
    assert_eq!(
        drawn(&session)
            .iter()
            .map(|s| (*s).to_owned())
            .collect::<Vec<_>>(),
        first_page,
        "翻回来该是原来那一页"
    );
}

#[test]
fn the_last_page_cannot_be_passed() {
    let Some(mut session) = ready() else {
        return;
    };
    type_text(&mut session, "shi");

    // 一路翻到底。翻页的边界是纯算术，这里直接调动作而不是点箭头——
    // 「点箭头能翻页」由上面那个测试覆盖，这个测试只管夹在首末页之间
    for _ in 0..300 {
        session.apply(Act::Page(1));
    }
    let footer = session.frame.footer.clone().expect("该有多页");
    let (page, pages) = footer.split_once('/').expect("页码形如 1/100");
    assert_eq!(page, pages, "翻到底该停在最后一页，实际 {footer}");
    let last_page: Vec<String> = drawn(&session).iter().map(|s| (*s).to_owned()).collect();

    session.apply(Act::Page(1));

    assert_eq!(
        drawn(&session)
            .iter()
            .map(|s| (*s).to_owned())
            .collect::<Vec<_>>(),
        last_page,
        "最后一页再往后翻该原地不动"
    );
    assert!(
        !last_page.is_empty(),
        "最后一页也该有候选（不该翻过头翻成空的）"
    );
}

#[test]
fn the_first_page_cannot_be_passed() {
    let Some(mut session) = ready() else {
        return;
    };
    type_text(&mut session, "shi");
    let first_page: Vec<String> = drawn(&session).iter().map(|s| (*s).to_owned()).collect();

    for _ in 0..3 {
        session.apply(Act::Page(-1));
    }

    assert_eq!(
        drawn(&session)
            .iter()
            .map(|s| (*s).to_owned())
            .collect::<Vec<_>>(),
        first_page,
        "第一页再往前翻该原地不动"
    );
}

#[test]
fn swiping_left_on_the_bar_pages_forward() {
    let Some(mut session) = ready() else {
        return;
    };
    type_text(&mut session, "shi");
    let (x, y) = (WIDTH * DENSITY / 2.0, session.bar_height() * DENSITY / 2.0);

    session.touch(MotionAction::Down, POINTER, x, y);
    session.touch(MotionAction::Move, POINTER, x - 120.0, y);
    session.touch(MotionAction::Up, POINTER, x - 120.0, y);

    assert!(
        session.frame.footer.as_deref().unwrap().starts_with("2/"),
        "往左划该翻到下一页，实际 {:?}",
        session.frame.footer
    );
}

#[test]
fn a_small_drag_on_the_bar_does_not_page() {
    let Some(mut session) = ready() else {
        return;
    };
    type_text(&mut session, "shi");
    let (x, y) = (WIDTH * DENSITY / 2.0, session.bar_height() * DENSITY / 2.0);
    let first_page: Vec<String> = drawn(&session).iter().map(|s| (*s).to_owned()).collect();
    // 拖的距离要挑在「点击容差」与「翻页阈值」之间：这里 60 像素，
    // 大于触摸阈值（8 点 × 2.75 ≈ 22 像素）算滑动，远小于翻页阈值（40 点 ≈ 110 像素）
    let drag = 60.0;

    session.touch(MotionAction::Down, POINTER, x, y);
    session.touch(MotionAction::Move, POINTER, x - drag, y);
    session.touch(MotionAction::Up, POINTER, x - drag, y);

    assert_eq!(session.take_commit(), None, "拖了就不该当成点了候选");
    assert!(
        session
            .frame
            .footer
            .as_deref()
            .unwrap_or_default()
            .starts_with("1/"),
        "也没划够远，该还停在第一页，实际 {:?}",
        session.frame.footer
    );
    assert_eq!(
        drawn(&session)
            .iter()
            .map(|s| (*s).to_owned())
            .collect::<Vec<_>>(),
        first_page,
        "候选不该变"
    );
}

#[test]
fn a_drag_on_the_keyboard_does_not_page() {
    let Some(mut session) = ready() else {
        return;
    };
    type_text(&mut session, "shi");
    let (x, y) = key_centre(&session, KeyId::Letter('a'));

    session.touch(MotionAction::Down, POINTER, x, y);
    session.touch(MotionAction::Move, POINTER, x - 120.0, y);
    session.touch(MotionAction::Up, POINTER, x - 120.0, y);

    assert_eq!(
        preedit(&session).as_deref(),
        Some("shi"),
        "在键盘上划走该什么也不做（不算按了 a，也不算翻页）"
    );
    assert!(session.frame.footer.as_deref().unwrap().starts_with("1/"));
}

#[test]
fn a_tap_on_the_bar_is_not_a_key_press() {
    let Some(mut session) = ready() else {
        return;
    };
    // 候选条那一段点空处（没有候选时哪块都不占）
    let (x, y) = (WIDTH * DENSITY / 2.0, session.bar_height() * DENSITY / 2.0);
    tap_at(&mut session, x, y);
    assert!(
        session.frame.preedit.is_none(),
        "候选条那一排不归任何键，不该出拼音"
    );
}

#[test]
fn the_bar_is_drawn_at_the_fixed_height() {
    let Some(mut session) = ready() else {
        return;
    };
    let bytes = session.bar_surface();
    assert!(bytes.len() > 8, "候选条该有位图");
    let height = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    assert_eq!(
        height,
        (session.bar_height() * DENSITY).round() as u32,
        "候选条高度必须是主题定死的那个值"
    );
}

#[test]
fn the_mode_key_switches_to_english_and_back() {
    let Some(mut session) = ready() else {
        return;
    };
    type_text(&mut session, "ni");
    assert!(preedit(&session).is_some(), "中文模式下该在组句");

    tap_key(&mut session, KeyId::Mode);

    assert!(session.english(), "该切到英文模式");
    assert!(
        session.frame.preedit.is_none(),
        "切换时该把没上屏的拼音丢掉，实际还剩 {:?}",
        preedit(&session)
    );

    // 英文模式直输：字母直接打出去，不进缓冲区、也没有候选
    type_text(&mut session, "hi");
    assert_eq!(
        session.take_commit().as_deref(),
        Some("hi"),
        "英文模式该把字母直接打出去"
    );
    assert!(preedit(&session).is_none(), "英文模式不该组句");
    assert!(drawn(&session).is_empty(), "英文模式还没有候选可给");

    // 切回中文，同一串又当拼音算
    tap_key(&mut session, KeyId::Mode);
    assert!(!session.english(), "该切回中文模式");
    type_text(&mut session, "ni");
    assert_eq!(preedit(&session).as_deref(), Some("ni"));
}

#[test]
fn shift_gives_uppercase_in_english_mode_only() {
    let Some(mut session) = ready() else {
        return;
    };
    // 中文模式：Shift 只影响键帽，喂进去的还是小写拼音
    tap_key(&mut session, KeyId::Shift);
    type_text(&mut session, "ni");
    assert_eq!(
        preedit(&session).as_deref(),
        Some("ni"),
        "中文模式下 Shift 不该改变拼音"
    );
    tap_key(&mut session, KeyId::Shift);

    // 英文模式：大小写跟着 Shift 走
    tap_key(&mut session, KeyId::Mode);
    type_text(&mut session, "hi");
    assert_eq!(
        session.take_commit().as_deref(),
        Some("hi"),
        "没锁定时该是小写"
    );

    tap_key(&mut session, KeyId::Shift);
    type_text(&mut session, "hi");
    assert_eq!(
        session.take_commit().as_deref(),
        Some("HI"),
        "锁了 Shift 该是大写"
    );
}

#[test]
fn english_mode_punctuation_stays_half_width() {
    let Some(mut session) = ready() else {
        return;
    };
    tap_key(&mut session, KeyId::Mode);

    tap_key(&mut session, KeyId::Comma);

    assert_eq!(
        session.take_commit().as_deref(),
        Some(","),
        "英文模式该打半角"
    );
}

#[test]
fn the_comma_key_commits_the_word_then_the_punctuation() {
    let Some(mut session) = ready() else {
        return;
    };
    type_text(&mut session, "nihao");

    tap_key(&mut session, KeyId::Comma);

    // 组句中打标点：先上屏高亮候选，再打标点，顺序不能反
    assert_eq!(
        session.take_commit().as_deref(),
        Some("你好，"),
        "该先上屏「你好」再打全角逗号"
    );
    assert!(session.frame.preedit.is_none(), "标点之后拼音该清空");
}

#[test]
fn the_comma_key_alone_is_just_a_punctuation() {
    let Some(mut session) = ready() else {
        return;
    };
    tap_key(&mut session, KeyId::Comma);
    assert_eq!(session.take_commit().as_deref(), Some("，"));
    assert!(session.take_commands().is_empty(), "标点不该走原样按键");
}

/// 随包资源目录，就是仓库的 `assets/emoji/`（emoji 字体与 emoji 表）。
fn bundle() -> Option<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = root.join("assets/emoji");
    dir.is_dir().then_some(dir)
}

#[test]
fn the_bundled_emoji_table_puts_emoji_in_the_candidates() {
    let (Some(dictionary), Some(bundle)) = (dictionary(), bundle()) else {
        return;
    };
    let mut session = Session::open(&dictionary, "zh-CN", Some(&bundle)).expect("会话该能打开");
    session.configure(WIDTH, DENSITY, 0.0, false);
    session.keyboard_surface();
    session.bar_surface();

    type_text(&mut session, "nihao");

    // emoji 可能排在后面几页，所以看整份候选而不是当前这一页
    let emoji: Vec<&str> = session
        .candidates
        .iter()
        .filter(|candidate| candidate.kind == CandidateKind::Emoji)
        .map(|candidate| candidate.text.as_str())
        .collect();
    assert!(
        !emoji.is_empty(),
        "带上随包的 emoji 表后该出 emoji 候选，实际一整份里一个都没有"
    );
}

/// 某个键的命中矩形（**整块输入视图的像素**：y 要加上候选条那一段）。
///
/// 布局里的字母键存的是大写（画的时候才按 Shift 转小写），所以这里先转过去比。
fn key_rect(session: &Session, id: KeyId) -> (f32, f32, f32, f32) {
    let id = match id {
        KeyId::Letter(c) => KeyId::Letter(c.to_ascii_uppercase()),
        other => other,
    };
    let key = session
        .keyboard
        .as_ref()
        .expect("键盘还没画过，没有命中矩形")
        .key_rect(id)
        .unwrap_or_else(|| panic!("键盘上没有 {id:?}"));
    let bar_pixels = session.bar_height() * session.density;
    (key.x, bar_pixels + key.y, key.width, key.height)
}

/// 按下 → 挪一点点 → 抬起，全程都在同一个键上。
fn tap_with_drift(session: &mut Session, id: KeyId, drift: f32) {
    let (x, y) = key_centre(session, id);
    session.touch(MotionAction::Down, POINTER, x, y);
    session.touch(MotionAction::Move, POINTER, x + drift, y);
    session.touch(MotionAction::Up, POINTER, x + drift, y);
    session.bar_surface();
    session.keyboard_surface();
}

#[test]
fn a_fast_tap_that_wobbles_a_few_pixels_still_counts() {
    // 真机上「点快了掉字母」就是这条：快敲时手指会挪几个像素，
    // 判定一紧就把整下敲击当成滑动取消掉了
    for drift in [1.0, 3.0, 6.0, 10.0, 20.0] {
        let Some(mut session) = ready() else {
            return;
        };
        tap_with_drift(&mut session, KeyId::Letter('n'), drift);
        assert_eq!(
            preedit(&session).as_deref(),
            Some("n"),
            "手指挪 {drift} 像素仍在同一个键上，这一下该算数"
        );
    }
}

#[test]
fn lifting_a_hair_past_the_key_edge_still_counts() {
    let Some(mut session) = ready() else {
        return;
    };
    let (x, y, width, _) = key_rect(&session, KeyId::Letter('a'));
    // 按在 a 的右边缘里侧，抬起时手指已经越过边缘落进键之间的缝——但只挪了十来像素，
    // 仍在触摸阈值内，这一下该算在 a 头上（安卓的键盘就是这么判的）
    session.touch(MotionAction::Down, POINTER, x + width - 2.0, y + 10.0);
    session.touch(MotionAction::Up, POINTER, x + width + 8.0, y + 10.0);
    session.bar_surface();
    session.keyboard_surface();
    assert_eq!(
        preedit(&session).as_deref(),
        Some("a"),
        "越过边缘一点点该还算在按下的那个键上"
    );
}

#[test]
fn sliding_over_to_another_key_cancels() {
    let Some(mut session) = ready() else {
        return;
    };
    let (ax, ay) = key_centre(&session, KeyId::Letter('a'));
    let (bx, by) = key_centre(&session, KeyId::Letter('s'));
    session.touch(MotionAction::Down, POINTER, ax, ay);
    session.touch(MotionAction::Move, POINTER, bx, by);
    session.touch(MotionAction::Up, POINTER, bx, by);
    session.bar_surface();
    session.keyboard_surface();
    assert!(
        session.frame.preedit.is_none(),
        "手指滑到隔壁键上该整下作废，实际出了 {:?}",
        preedit(&session)
    );
}

#[test]
fn the_touch_threshold_scales_with_density() {
    let Some(session) = ready() else {
        return;
    };
    // 8 点 × 2.75 ≈ 22 像素。要是忘了乘密度就只剩 8 像素，快敲必然误判
    assert!(
        (session.touch_slop() - 8.0 * DENSITY).abs() < 0.01,
        "触摸阈值该按密度换算，实际 {}",
        session.touch_slop()
    );
}

#[test]
fn two_fingers_overlapping_do_not_eat_each_other() {
    // 快打时两根拇指的接触时间会重叠，安卓这时发的是 POINTER_DOWN / POINTER_UP（各指一根手指），
    // 不是 DOWN / UP。之前把这两个当成取消，一重叠就是两根的字母一起丢——
    // 真机上「点快了有很多字母会略过」主要就是它。
    let Some(mut session) = ready() else {
        return;
    };
    let (nx, ny) = key_centre(&session, KeyId::Letter('n'));
    let (ix, iy) = key_centre(&session, KeyId::Letter('i'));

    session.touch(MotionAction::Down, 0, nx, ny); // 拇指 A 按 n
    session.touch(MotionAction::PointerDown, 1, ix, iy); // 拇指 B 在 A 抬起前按 i
    session.touch(MotionAction::PointerUp, 0, nx, ny); // A 抬起
    session.touch(MotionAction::Up, 1, ix, iy); // B 抬起

    assert_eq!(
        preedit(&session).as_deref(),
        Some("ni"),
        "两根手指叠着敲，两个字母都该出来"
    );
}

#[test]
fn overlapping_fingers_keep_their_own_letter_when_lifted_in_the_other_order() {
    let Some(mut session) = ready() else {
        return;
    };
    let (nx, ny) = key_centre(&session, KeyId::Letter('n'));
    let (ax, ay) = key_centre(&session, KeyId::Letter('a'));

    session.touch(MotionAction::Down, 0, nx, ny);
    session.touch(MotionAction::PointerDown, 1, ax, ay);
    session.touch(MotionAction::PointerUp, 1, ax, ay); // 后按下的先抬
    session.touch(MotionAction::Up, 0, nx, ny);

    assert_eq!(
        preedit(&session).as_deref(),
        Some("an"),
        "按下的先后与抬起的先后不一致时，也各出各的字母"
    );
}

#[test]
fn a_slide_from_one_finger_does_not_cancel_another() {
    let Some(mut session) = ready() else {
        return;
    };
    let (nx, ny) = key_centre(&session, KeyId::Letter('n'));
    let (ax, ay) = key_centre(&session, KeyId::Letter('a'));
    let (sx, sy) = key_centre(&session, KeyId::Letter('s'));

    session.touch(MotionAction::Down, 0, nx, ny);
    session.touch(MotionAction::PointerDown, 1, ax, ay);
    // 第二根手指滑到别处（这一下自己作废），但不该影响第一根
    session.touch(MotionAction::Move, 1, sx, sy);
    session.touch(MotionAction::PointerUp, 1, sx, sy);
    session.touch(MotionAction::Up, 0, nx, ny);

    assert_eq!(
        preedit(&session).as_deref(),
        Some("n"),
        "一根手指滑走，不该把另一根已经按下的字母也带走"
    );
}

/// 三页走一圈：字母 →数字 →符号 →数字 →字母，每页上的键都点得着。
///
/// `tap_key` 的点位是从**当前渲染出来的命中矩形**里取的，找不到那个键就 panic——
/// 所以「点着了」本身就是「这一页真的换过去了」的证据。
#[test]
fn the_panels_can_be_walked_all_the_way_around() {
    let Some(mut session) = ready() else {
        return;
    };

    tap_key(&mut session, KeyId::Panel(Panel::Digits));
    tap_key(&mut session, KeyId::Literal('1'));

    tap_key(&mut session, KeyId::Panel(Panel::Symbols));
    tap_key(&mut session, KeyId::Literal('@'));

    tap_key(&mut session, KeyId::Panel(Panel::Digits));
    tap_key(&mut session, KeyId::Panel(Panel::Letters));
    tap_key(&mut session, KeyId::Letter('n'));

    assert_eq!(
        session.take_commit().as_deref(),
        Some("1@"),
        "数字页与符号页打出来的该一前一后都在"
    );
    assert_eq!(
        preedit(&session).as_deref(),
        Some("n"),
        "回到字母页还能接着打拼音"
    );
}

/// 符号在中文模式下出全角——引擎那张表转的，壳这边只报半角原字符。
#[test]
fn a_symbol_comes_out_full_width_in_chinese_mode() {
    let Some(mut session) = ready() else {
        return;
    };

    tap_key(&mut session, KeyId::Panel(Panel::Digits));
    tap_key(&mut session, KeyId::Panel(Panel::Symbols));
    tap_key(&mut session, KeyId::Literal('?'));

    assert_eq!(session.take_commit().as_deref(), Some("？"));
}

/// 数字保持半角；数字后面那个点也保持半角（引擎的规矩）。
#[test]
fn digits_and_a_trailing_dot_stay_half_width() {
    let Some(mut session) = ready() else {
        return;
    };

    tap_key(&mut session, KeyId::Panel(Panel::Digits));
    tap_key(&mut session, KeyId::Literal('3'));
    // 小数点住在符号页
    tap_key(&mut session, KeyId::Panel(Panel::Symbols));
    tap_key(&mut session, KeyId::Literal('.'));
    tap_key(&mut session, KeyId::Panel(Panel::Digits));
    tap_key(&mut session, KeyId::Literal('1'));

    assert_eq!(
        session.take_commit().as_deref(),
        Some("3.1"),
        "数字后面那个点该保持半角，中文模式下也不转成句号"
    );
}

/// 组句时敲数字：先把高亮候选上屏，再打数字——跟敲逗号一个规矩。
#[test]
fn a_digit_while_composing_commits_the_word_first() {
    let Some(mut session) = ready() else {
        return;
    };
    type_text(&mut session, "nihao");
    let first = drawn(&session)
        .first()
        .map(|text| (*text).to_owned())
        .expect("该有候选");

    tap_key(&mut session, KeyId::Panel(Panel::Digits));
    tap_key(&mut session, KeyId::Literal('1'));

    assert_eq!(
        session.take_commit().as_deref(),
        Some(format!("{first}1").as_str()),
        "该先上屏「{first}」再打数字"
    );
}

/// 换应用（`clear`）之后键盘回到字母页——键盘收起来再弹出来该从字母页开始。
///
/// 判法是「点得着 / 点不着」：`tap_key` 的点位从**当前渲染出来的命中矩形**里取，找不到就 panic。
#[test]
fn switching_apps_puts_the_keyboard_back_on_the_letters_page() {
    let Some(mut session) = ready() else {
        return;
    };

    tap_key(&mut session, KeyId::Panel(Panel::Digits));
    tap_key(&mut session, KeyId::Literal('1'));
    assert_eq!(session.take_commit().as_deref(), Some("1"), "先在数字页");

    // 换应用：壳在 onFinishInput 里调它，再照回来的掩码重画键盘（这里是同一套动作）
    let flags = session.clear();
    assert_eq!(flags & 2, 2, "掩码里该带 FLAG_KEYBOARD，壳据此重画");
    session.keyboard_surface();

    tap_key(&mut session, KeyId::Letter('n'));
    assert_eq!(
        preedit(&session).as_deref(),
        Some("n"),
        "回到字母页，字母键点得着"
    );
}

/// 候选条上那个 ×（`Act::Clear`）只清拼音，**不动页**——跟换应用不是一回事。
#[test]
fn the_clear_button_does_not_change_the_panel() {
    let Some(mut session) = ready() else {
        return;
    };

    // 候选条上那个 × 只在**有拼音**时才存在，先打一段拼音；切页不动拼音
    type_text(&mut session, "nihao");
    tap_key(&mut session, KeyId::Panel(Panel::Digits));

    tap_bar(&mut session, BarHitId::Clear);
    session.keyboard_surface();

    tap_key(&mut session, KeyId::Literal('8'));
    assert_eq!(
        session.take_commit().as_deref(),
        Some("8"),
        "还在数字页，数字键还点得着"
    );
}

/// 键盘收起来再弹出来（`onStartInputView` → `reset_panel`）回字母页，**但拼音不动**。
///
/// 跟 `clear`（换应用）分开就是这个道理：收起键盘没有结束输入，拼音该留着。
#[test]
fn showing_the_keyboard_again_goes_back_to_letters_without_dropping_the_pinyin() {
    let Some(mut session) = ready() else {
        return;
    };
    type_text(&mut session, "nihao");
    tap_key(&mut session, KeyId::Panel(Panel::Digits));

    let flags = session.reset_panel();
    assert_eq!(flags & 2, 2, "掩码里该带 FLAG_KEYBOARD，壳据此重画");
    session.keyboard_surface();

    assert_eq!(
        preedit(&session).as_deref(),
        Some("ni'hao"),
        "收起键盘不该丢掉拼音"
    );
    tap_key(&mut session, KeyId::Letter('n'));
    assert_eq!(
        preedit(&session).as_deref(),
        Some("ni'hao'n"),
        "回到字母页接着打（n 起了新音节，引擎自己补 '）"
    );
}
