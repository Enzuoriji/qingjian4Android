//! 会话的端到端测试：**在电脑上跑，不需要模拟器或真机**。
//!
//! 喂的是真实坐标，走的是完整触摸链路——点位从渲染器真正画出来的命中矩形里取，
//! 所以布局一改、命中算错，这里先炸。「能打字能选词」这条验收因此是可断言的。

use super::Session;
use crate::action::{Act, Command};
use crate::touch::MotionAction;
use qingjian_render::{BarHitId, KeyId};
use std::path::PathBuf;

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
///
/// 布局里的字母键存的是大写（画的时候才按 Shift 转小写），所以这里先转过去比。
fn key_centre(session: &Session, id: KeyId) -> (f32, f32) {
    let id = match id {
        KeyId::Letter(c) => KeyId::Letter(c.to_ascii_uppercase()),
        other => other,
    };
    let keyboard = session
        .keyboard
        .as_ref()
        .expect("键盘还没画过，没有命中矩形");
    let key = keyboard
        .keys
        .iter()
        .find(|key| key.id == id)
        .unwrap_or_else(|| panic!("键盘上没有 {id:?}"));
    // y 从候选条底下开始——壳传进来的就是整块输入视图的坐标
    let bar_pixels = session.bar_height() * session.density;
    (
        key.x + key.width / 2.0,
        bar_pixels + key.y + key.height / 2.0,
    )
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
    session.touch(MotionAction::Down, x, y);
    session.touch(MotionAction::Up, x, y);
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
    let mut session = Session::open(&dictionary()?, "zh-CN").ok()?;
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

    session.touch(MotionAction::Down, x, y);
    session.touch(MotionAction::Move, x - 120.0, y);
    session.touch(MotionAction::Up, x - 120.0, y);

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

    session.touch(MotionAction::Down, x, y);
    session.touch(MotionAction::Move, x - 20.0, y);
    session.touch(MotionAction::Up, x - 20.0, y);

    assert!(
        session.frame.footer.as_deref().unwrap().starts_with("1/"),
        "只挪一点点该还停在第一页，实际 {:?}",
        session.frame.footer
    );
}

#[test]
fn a_drag_on_the_keyboard_does_not_page() {
    let Some(mut session) = ready() else {
        return;
    };
    type_text(&mut session, "shi");
    let (x, y) = key_centre(&session, KeyId::Letter('a'));

    session.touch(MotionAction::Down, x, y);
    session.touch(MotionAction::Move, x - 120.0, y);
    session.touch(MotionAction::Up, x - 120.0, y);

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
