//! 会话的端到端测试：**在电脑上跑，不需要模拟器或真机**。
//!
//! 喂的是真实坐标，走的是完整触摸链路——点位从渲染器真正画出来的命中矩形里取，
//! 所以布局一改、命中算错，这里先炸。「能打字能选词」这条验收因此是可断言的。

use super::Session;
use super::flags;
use crate::action::{Act, Command};
use crate::touch::MotionAction;
use qingjian_core::CandidateKind;
use qingjian_render::{BarHitId, CLIPBOARD_CELLS, KeyId, Panel};
use std::path::PathBuf;

/// 单指测试用的 pointer id。多点触控的用例自己给别的编号。
const POINTER: i32 = 0;

/// 验收用的屏幕宽（点）与密度，按一台常见手机竖屏。
const WIDTH: f32 = 360.0;
const DENSITY: f32 = 2.75;

/// 屏幕在**当前方向**上的高度（点）。这台手机是 360 × 800，所以竖屏 800、横屏 360。
///
/// 键盘高度按它算（屏幕大的手机键盘也大），所以这两个数得跟着方向换——
/// 拿同一个高度去比「横屏矮一截」，验的就不是真机上的那回事了。
const PORTRAIT_HEIGHT: f32 = 800.0;
const LANDSCAPE_HEIGHT: f32 = 360.0;

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

/// 齿轮 → 工具页 → 剪贴板页，最后停在那页上。
fn open_clipboard(session: &mut Session) {
    tap_bar(session, BarHitId::Tools);
    tap_key(session, KeyId::Tool(0));
}

/// 在剪贴板记录区拖一段再松手（一次完整的滚动）。`dy` 正数往下。
///
/// 滚动是**跟手**的，所以是一段段 Move（每段都按同一套阈值判方向），不是一步到位。
fn swipe_vertically(session: &mut Session, dy: f32) {
    let (x, y) = key_centre(session, KeyId::Clipboard(0));
    session.touch(MotionAction::Down, POINTER, x, y);
    let step = 10.0 * DENSITY;
    let mut moved = 0.0;
    while moved < dy.abs() {
        moved = (moved + step).min(dy.abs());
        session.touch(MotionAction::Move, POINTER, x, y + moved * dy.signum());
    }
    session.touch(MotionAction::Up, POINTER, x, y + moved * dy.signum());
    session.keyboard_surface();
}

/// 往上拖：看后面的条目。
fn scroll_up(session: &mut Session, distance: f32) {
    swipe_vertically(session, -distance);
}

/// 往下拖：往回看最新的那几条。
fn scroll_down(session: &mut Session, distance: f32) {
    swipe_vertically(session, distance);
}

/// 在一格上**往左滑**（过阈值）再松手。
fn swipe_left(session: &mut Session, id: KeyId) {
    let (x, y) = key_centre(session, id);
    let swipe = 16.0 * DENSITY + 20.0;
    session.touch(MotionAction::Down, POINTER, x, y);
    session.touch(MotionAction::Move, POINTER, x - swipe, y);
    session.touch(MotionAction::Up, POINTER, x - swipe, y);
    session.keyboard_surface();
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
    session.configure(WIDTH, PORTRAIT_HEIGHT, DENSITY, 0.0, false, false);
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
fn paging_shows_the_next_screen() {
    let Some(mut session) = ready() else {
        return;
    };
    // 「shi」在词库里有一大把候选，够翻的
    type_text(&mut session, "shi");
    let footer = session.frame.footer.clone().expect("不止一屏就该有页码");
    assert!(footer.starts_with("1/"), "页码该从第一屏起，实际 {footer}");
    let first_screen: Vec<String> = drawn(&session).iter().map(|s| (*s).to_owned()).collect();

    tap_bar(&mut session, BarHitId::PageNext);

    assert!(
        session.frame.footer.as_deref().unwrap().starts_with("2/"),
        "该翻到第二屏，实际 {:?}",
        session.frame.footer
    );
    let second_screen: Vec<String> = drawn(&session).iter().map(|s| (*s).to_owned()).collect();
    assert_ne!(first_screen, second_screen, "第二屏该是别的候选");

    tap_bar(&mut session, BarHitId::PagePrev);
    assert_eq!(
        drawn(&session)
            .iter()
            .map(|s| (*s).to_owned())
            .collect::<Vec<_>>(),
        first_screen,
        "翻回来该是原来那一屏"
    );
}

/// **页码跟着手指走**——用户报的就是这个：滑了半天数字不动。
///
/// 2026-09-21 之前页码是「第几批 / 共几批」，一批 24 个候选；一屏看得见十来个，
/// 所以要滑满三屏多才跳一格，看着就像坏了。现在带子是通的，页码说的是屏。
#[test]
fn the_page_number_follows_the_finger() {
    let Some(mut session) = ready() else {
        return;
    };
    type_text(&mut session, "shi");
    let viewport = WIDTH * DENSITY;
    let page = |session: &Session| session.frame.footer.clone().unwrap_or_default();

    assert!(page(&session).starts_with("1/"), "该从第一屏起");

    let (x, _) = bar_centre(&session, BarHitId::Candidate(0));
    let y = bar_y(&session);
    session.touch(MotionAction::Down, POINTER, x, y);

    // 不到一屏：还在第一屏
    session.touch(MotionAction::Move, POINTER, x - viewport * 0.4, y);
    assert!(
        page(&session).starts_with("1/"),
        "半屏都还没滑到，该还在第一屏，实际 {}",
        page(&session)
    );

    // 过了一屏：第二屏
    session.touch(MotionAction::Move, POINTER, x - viewport * 1.1, y);
    assert!(
        page(&session).starts_with("2/"),
        "滑过一屏就该报第二屏，实际 {}",
        page(&session)
    );

    session.touch(MotionAction::Up, POINTER, x - viewport * 1.1, y);
}

#[test]
fn the_last_screen_cannot_be_passed() {
    let Some(mut session) = ready() else {
        return;
    };
    type_text(&mut session, "shi");

    // 一路翻到底。翻页的边界是纯算术，这里直接调动作而不是点箭头——
    // 「点箭头能翻页」由上面那个测试覆盖，这个测试只管夹在首末屏之间
    for _ in 0..300 {
        session.apply(Act::Page(1));
    }
    let footer = session.frame.footer.clone().expect("该有多屏");
    let (screen, screens) = footer.split_once('/').expect("页码形如 1/100");
    assert_eq!(
        screen, screens,
        "翻到底该停在最后一屏——分子走不到分母的话，页码会停在 24/25 那种数上，实际 {footer}"
    );
    let last_screen: Vec<String> = drawn(&session).iter().map(|s| (*s).to_owned()).collect();

    session.apply(Act::Page(1));

    assert_eq!(
        drawn(&session)
            .iter()
            .map(|s| (*s).to_owned())
            .collect::<Vec<_>>(),
        last_screen,
        "最后一屏再往后翻该原地不动"
    );
    assert!(
        !last_screen.is_empty(),
        "最后一屏也该有候选（不该翻过头翻成空的）"
    );
}

#[test]
fn the_first_screen_cannot_be_passed() {
    let Some(mut session) = ready() else {
        return;
    };
    type_text(&mut session, "shi");
    let first_screen: Vec<String> = drawn(&session).iter().map(|s| (*s).to_owned()).collect();

    for _ in 0..3 {
        session.apply(Act::Page(-1));
    }

    assert_eq!(
        drawn(&session)
            .iter()
            .map(|s| (*s).to_owned())
            .collect::<Vec<_>>(),
        first_screen,
        "第一屏再往前翻该原地不动"
    );
}

/// 候选条上某一块的中心（按下 / 抬起用）。与 `bar_centre` 同源，只是这里要按需取。
fn bar_y(session: &Session) -> f32 {
    session.bar_height() * DENSITY / 2.0
}

/// 候选条上某一块的**左边缘**（像素）——拖过之后看它挪没挪。
fn bar_left(session: &Session, id: BarHitId) -> f32 {
    session
        .bar
        .as_ref()
        .expect("候选条还没画过")
        .hits
        .iter()
        .find(|hit| hit.id == id)
        .expect("候选条上没有这一块")
        .x
}

/// **候选条能横滚**：按住拖着走，那条带子跟着手指平移。
///
/// 这是 2026-09-21 换掉「横滑翻页」之后的手势——**拖动只是看，不上屏**；
/// 要选还得点一下（或空格上屏本页第一个）。要「滚到哪儿就选哪个」的话，
/// 一路翻看过去就没法全身而退了。
#[test]
fn dragging_the_bar_scrolls_it_and_does_not_select() {
    let Some(mut session) = ready() else {
        return;
    };
    type_text(&mut session, "shi");
    let before = bar_left(&session, BarHitId::Candidate(0));
    let (x, _) = bar_centre(&session, BarHitId::Candidate(0));
    let y = bar_y(&session);
    let drag = 120.0;

    session.touch(MotionAction::Down, POINTER, x, y);
    session.touch(MotionAction::Move, POINTER, x - drag, y);
    session.bar_surface(); // 重画一次，命中矩形才跟上

    let after = bar_left(&session, BarHitId::Candidate(0));
    assert!(
        (after - (before - drag)).abs() < 0.01,
        "带子该跟手走：从 {before} 拖 {drag} 之后该到 {}，实际 {after}",
        before - drag
    );

    session.touch(MotionAction::Up, POINTER, x - drag, y);

    assert_eq!(session.take_commit(), None, "拖动只是滚，不该上屏");
    assert_eq!(
        preedit(&session).as_deref(),
        Some("shi"),
        "拼音该原样留着，接着看或接着打"
    );
}

/// 拖一段候选条，然后在 `dx` 处抬手（返回按下那点的坐标与 y）。
///
/// 上屏的东西一概不兑现——拖动只是看。速度由壳量好报上来，所以这里单独调
/// [`Session::start_fling`]，不走触摸。
fn drag_then_lift(session: &mut Session, dx: f32) {
    let (x, _) = bar_centre(session, BarHitId::Candidate(0));
    let y = bar_y(session);
    session.touch(MotionAction::Down, POINTER, x, y);
    session.touch(MotionAction::Move, POINTER, x - dx, y);
    session.touch(MotionAction::Up, POINTER, x - dx, y);
    session.bar_surface();
}

/// 甩一把：手指抬了之后带子自己接着滑，一直滑到 Rust 说停。
#[test]
fn a_flick_glides_on_after_the_finger_leaves() {
    let Some(mut session) = ready() else {
        return;
    };
    type_text(&mut session, "shi");
    let viewport = WIDTH * DENSITY;
    drag_then_lift(&mut session, viewport * 0.2);

    // 抬手速度 2000 像素/秒、往左甩（向右为正，所以是负的）
    let flags = session.start_fling(POINTER, -2000.0);
    assert_eq!(flags & flags::FLING, flags::FLING, "够快就该甩起来");
    assert!(
        session
            .frame
            .footer
            .as_deref()
            .unwrap_or_default()
            .starts_with("1/"),
        "起手该在第一屏"
    );

    let mut frames = 0;
    while session.fling_step(16.0) & flags::FLING != 0 {
        frames += 1;
        assert!(frames < 200, "滑了两百帧还没停，衰减写错了");
    }
    assert!(frames > 3, "甩一把该滑好几帧，实际 {frames}");
    assert!(
        !session
            .frame
            .footer
            .as_deref()
            .unwrap_or_default()
            .starts_with("1/"),
        "该滑出去不止一屏，实际 {:?}",
        session.frame.footer
    );
}

/// 慢慢拖着松手：手指停在哪就是哪，不接着跑。
#[test]
fn a_slow_lift_does_not_glide() {
    let Some(mut session) = ready() else {
        return;
    };
    type_text(&mut session, "shi");
    drag_then_lift(&mut session, 120.0);

    // 100 像素/秒：比「甩」那条线（200）低
    let flags = session.start_fling(POINTER, -100.0);
    assert_eq!(flags & flags::FLING, 0, "慢慢松手不该自己跑起来");
    let before = drawn(&session)
        .iter()
        .map(|s| (*s).to_owned())
        .collect::<Vec<_>>();
    session.fling_step(16.0);
    assert_eq!(
        drawn(&session)
            .iter()
            .map(|s| (*s).to_owned())
            .collect::<Vec<_>>(),
        before,
        "没甩起来就一帧都不该动"
    );
}

/// 在候选条上点一下（没滑）再快抬：那不是甩——壳对每一下都会报速度上来。
#[test]
fn a_tap_on_the_bar_does_not_glide() {
    let Some(mut session) = ready() else {
        return;
    };
    type_text(&mut session, "shi");
    // 点在拼音行那块空处：不命中任何目标，也就不会上屏
    let y = bar_y(&session);
    session.touch(MotionAction::Down, POINTER, 120.0, y);
    session.touch(MotionAction::Up, POINTER, 120.0, y);

    let flags = session.start_fling(POINTER, -3000.0);
    assert_eq!(flags & flags::FLING, 0, "点一下不是甩，速度再快也不算");
}

/// 滑行当中手指一落下就停住——想抓回来抓得住。
#[test]
fn a_new_touch_stops_the_glide() {
    let Some(mut session) = ready() else {
        return;
    };
    type_text(&mut session, "shi");
    drag_then_lift(&mut session, 120.0);
    assert_ne!(
        session.start_fling(POINTER, -2000.0) & flags::FLING,
        0,
        "先得甩起来"
    );

    let (x, _) = bar_centre(&session, BarHitId::Candidate(0));
    let y = bar_y(&session);
    let flags = session.touch(MotionAction::Down, POINTER, x, y);
    assert_eq!(flags & flags::FLING, 0, "手指一落下，滑行就该停");

    let before = session.scroll;
    session.fling_step(16.0);
    assert_eq!(session.scroll, before, "停了就不该再动");
}

/// 候选换了（清空 / 上屏 / 再敲一个字母）滑行就停：那一段是接着「刚才滚到哪儿」走的。
#[test]
fn a_new_composition_stops_the_glide() {
    let Some(mut session) = ready() else {
        return;
    };
    type_text(&mut session, "shi");
    drag_then_lift(&mut session, 120.0);
    assert_ne!(
        session.start_fling(POINTER, -2000.0) & flags::FLING,
        0,
        "先得甩起来"
    );

    session.apply(Act::Clear);

    assert_eq!(
        session.fling_step(16.0) & flags::FLING,
        0,
        "候选一换，滑行就该停"
    );
    assert_eq!(session.scroll, 0.0, "而且带子该回到头上");
}

/// 一路滑到底就停住，不会在尽头空转。
#[test]
fn the_glide_stops_at_the_end_of_the_strip() {
    let Some(mut session) = ready() else {
        return;
    };
    type_text(&mut session, "shi");
    drag_then_lift(&mut session, 120.0);
    // 比安卓能给的最大速度还快：一定要够到带子尾巴
    assert_ne!(
        session.start_fling(POINTER, -200_000.0) & flags::FLING,
        0,
        "先得甩起来"
    );

    let mut frames = 0;
    while session.fling_step(16.0) & flags::FLING != 0 {
        frames += 1;
        assert!(frames < 400, "滑了 {frames} 帧还没停");
    }

    let footer = session.frame.footer.clone().expect("该有多屏");
    let (screen, screens) = footer.split_once('/').expect("页码形如 1/100");
    assert_eq!(screen, screens, "该正好停在最后一屏，实际 {footer}");
}

/// 滚过之后点候选，上屏的得是**手指底下那个**。
///
/// 命中矩形报的下标是「画出来这一批里的第几个」（从最左边看得见的那个数起），
/// 换回整份候选表要加上滚过去的那一段——错一格就会上屏隔壁的词。
#[test]
fn tapping_after_scrolling_commits_the_one_under_the_finger() {
    let Some(mut session) = ready() else {
        return;
    };
    type_text(&mut session, "shi");
    let viewport = WIDTH * DENSITY;
    let (x, _) = bar_centre(&session, BarHitId::Candidate(0));
    let y = bar_y(&session);

    // 拖过两屏多，停在半路上（没对齐屏边界）
    session.touch(MotionAction::Down, POINTER, x, y);
    session.touch(MotionAction::Move, POINTER, x - viewport * 2.4, y);
    session.touch(MotionAction::Up, POINTER, x - viewport * 2.4, y);
    session.bar_surface();

    // 点第三个——头一个可能只露半个在屏幕外
    let expected = drawn(&session)[2].to_owned();
    let (tx, _) = bar_centre(&session, BarHitId::Candidate(2));
    tap_at(&mut session, tx, y);

    assert_eq!(
        session.take_commit().as_deref(),
        Some(expected.as_str()),
        "点哪个上屏哪个，不该串到隔壁"
    );
}

/// 滚过之后空格上屏的，得是**第一个整个看得见的**候选——不是左边只露一条边那个。
///
/// 押的是「高亮认整格」：停在半格上时，最左边那格左边被切掉了，高亮该往后挪一格。
/// 不挪的话屏幕上只有一条蓝边，空格上屏的却是个几乎看不见的词。
#[test]
fn space_commits_the_first_whole_candidate_after_scrolling() {
    let Some(mut session) = ready() else {
        return;
    };
    type_text(&mut session, "shi");
    let viewport = WIDTH * DENSITY;
    let (x, _) = bar_centre(&session, BarHitId::Candidate(0));
    let y = bar_y(&session);

    // 拖的距离带个零头，好停在一格的中间而不是格缝上
    let drag = viewport * 0.6 + 17.0;
    session.touch(MotionAction::Down, POINTER, x, y);
    session.touch(MotionAction::Move, POINTER, x - drag, y);
    session.touch(MotionAction::Up, POINTER, x - drag, y);
    session.bar_surface();

    let cut = bar_left(&session, BarHitId::Candidate(0));
    assert!(cut < 0.0, "该停在半格上：第一格左边该是负的，实际 {cut}");

    let whole = drawn(&session)[1].to_owned();
    session.apply(Act::CommitHighlighted);

    assert_eq!(
        session.take_commit().as_deref(),
        Some(whole.as_str()),
        "空格该上屏第一个整格"
    );
}

/// 一路往左拖过**一整屏**，页码就该走到第二屏。
#[test]
fn dragging_a_whole_screen_turns_the_page() {
    let Some(mut session) = ready() else {
        return;
    };
    type_text(&mut session, "shi");
    let viewport = WIDTH * DENSITY;
    let (x, _) = bar_centre(&session, BarHitId::Candidate(0));
    let y = bar_y(&session);

    session.touch(MotionAction::Down, POINTER, x, y);
    // 拖过一整屏还多一点：多出来的那点留在第二屏上，接着滑
    session.touch(MotionAction::Move, POINTER, x - viewport - 30.0, y);

    assert!(
        session
            .frame
            .footer
            .as_deref()
            .unwrap_or_default()
            .starts_with("2/"),
        "拖过一整屏该翻到第二屏，实际 {:?}",
        session.frame.footer
    );
}

/// 往右拖回一整屏，页码就该走回第一屏。
#[test]
fn dragging_back_a_screen_turns_back_a_page() {
    let Some(mut session) = ready() else {
        return;
    };
    type_text(&mut session, "shi");
    let viewport = WIDTH * DENSITY;
    let (x, _) = bar_centre(&session, BarHitId::Candidate(0));
    let y = bar_y(&session);

    // 先翻到第二屏，再往回拖
    tap_bar(&mut session, BarHitId::PageNext);
    assert!(
        session
            .frame
            .footer
            .as_deref()
            .unwrap_or_default()
            .starts_with("2/"),
        "先该在第二屏"
    );

    session.touch(MotionAction::Down, POINTER, x, y);
    session.touch(MotionAction::Move, POINTER, x + viewport - 30.0, y);

    assert!(
        session
            .frame
            .footer
            .as_deref()
            .unwrap_or_default()
            .starts_with("1/"),
        "往回拖一整屏该回到第一屏，实际 {:?}",
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

    session.touch(MotionAction::Down, POINTER, x, y);
    session.touch(MotionAction::Move, POINTER, x - 120.0, y);
    session.touch(MotionAction::Up, POINTER, x - 120.0, y);

    // 横着划走够远：手指出了键，这一下**作废**——既不出字母也不出角标
    // （角标现在只认往下滑，见 `the_hint_only_comes_out_when_you_swipe_down`）。
    assert_eq!(session.take_commit(), None, "键盘上横划不该上屏任何东西");
    assert!(
        session
            .frame
            .footer
            .as_deref()
            .unwrap_or_default()
            .starts_with("1/"),
        "**翻页是候选条的手势**，在键盘上横划不该翻它的页，实际 {:?}",
        session.frame.footer
    );
}

#[test]
fn a_tap_on_the_bar_is_not_a_key_press() {
    let Some(mut session) = ready() else {
        return;
    };
    // 先敲一个字母把候选条顶出来——**没组句时这一条根本不存在**，没有可点的空处
    type_text(&mut session, "n");
    let before = preedit(&session);

    // 点在拼音行那块空白上——避开右端的清空 / 翻页按钮，也避开下方的候选格子
    // （点到格子会上屏候选，那是另一回事，不是这条要验的）
    let y = 10.0 * DENSITY;
    tap_at(&mut session, 100.0, y);

    assert_eq!(
        preedit(&session),
        before,
        "候选条那一排不归任何键，不该出拼音"
    );
}

/// 那条候选条**两种画法共用一块地方**：没组句时细细一条、里面一个齿轮；
/// 一打字候选条整个接管（齿轮让开），打字的竖向空间一点没多占。
#[test]
fn the_bar_shrinks_to_a_gear_strip_when_not_composing() {
    let Some(mut session) = ready() else {
        return;
    };

    // 没组句：一条细的，位图照出，那一块地方只有齿轮可点
    let idle = session.bar_height();
    assert!(idle > 0.0, "没组句时也该留一条细的（齿轮要有地方）");
    let bytes = session.bar_surface();
    assert!(bytes.len() > 8, "那条细的也得有位图");
    let height = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    assert_eq!(
        height,
        (idle * DENSITY).round() as u32,
        "位图高度该跟报出去的一致"
    );
    assert_eq!(gears(&session), 1, "没组句时只有齿轮一个可点的");

    type_text(&mut session, "nihao");

    // 组句：候选条接管，高度是主题定死的那个值（候选从 0 个变 6 个也不动）
    // （那条细的 2026-09-21 从 30 抬到 36，所以这里就按「长高一截」比，不按倍数）
    assert!(
        session.bar_height() > idle * 1.5,
        "一打字候选条该把那一块接管过去"
    );
    let bytes = session.bar_surface();
    let height = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    assert_eq!(
        height,
        (session.bar_height() * DENSITY).round() as u32,
        "组句时的高度该是主题定死的那个值"
    );
    assert_eq!(gears(&session), 0, "组句时齿轮让开，那块地方全给候选");
}

/// 候选条上「齿轮」那个命中区有几个（0 或 1）。
fn gears(session: &Session) -> usize {
    session.bar.as_ref().map_or(0, |bar| {
        bar.hits
            .iter()
            .filter(|hit| hit.id == BarHitId::Tools)
            .count()
    })
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
    session.configure(WIDTH, PORTRAIT_HEIGHT, DENSITY, 0.0, false, false);
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

/// 在 `id` 这个键上按下，往 `(dx, dy)` 方向滑，再抬起。
fn drag(session: &mut Session, id: KeyId, dx: f32, dy: f32) {
    let (x, y) = key_centre(session, id);
    session.touch(MotionAction::Down, POINTER, x, y);
    session.touch(MotionAction::Move, POINTER, x + dx, y + dy);
    session.touch(MotionAction::Up, POINTER, x + dx, y + dy);
    session.bar_surface();
    session.keyboard_surface();
}

/// 往下滑——`drag` 的常用写法。
fn swipe_down(session: &mut Session, id: KeyId, dy: f32) {
    drag(session, id, 0.0, dy);
}

/// 够滑动阈值那么多像素。**比的是纵向**——往下的位移到这儿才兑现角标。
///
/// 测试里**别正好用这个数去要求出角标**：位移是 `(y + d) - y` 算出来的，f32 下未必正好
/// 等于 `d`，卡在阈值上就成了浮点运气（实测同一批里两个键一个过一个不过）。
/// 要「这一下该出角标」就用 [`swipe_past`]；要「差一点、不该出」就用它减一。
fn swipe_distance() -> f32 {
    crate::keyboard::SWIPE * DENSITY
}

/// 手指挪几个像素不算滑动——快敲时手指本来就会歪一点。
///
/// 阈值定小了这条就会挂：正常打字全变成打符号，那是灾难。
#[test]
fn a_wobble_still_types_the_letter() {
    let drift = swipe_distance() - 1.0;
    // **横向只能挪到键里边为止**：挪出键外这一下就作废了（滑到隔壁键上要能反悔，
    // 见 `sliding_over_to_another_key_cancels`），而键只有三十来点宽——半键还不到 `drift`。
    // 纵向宽松得多：阈值现在比的是**键高**（角标只认往下滑），22 点仍落在键内。
    let half_key = {
        let Some(session) = ready() else {
            return;
        };
        let (_, _, width, _) = key_rect(&session, KeyId::Letter('q'));
        width / 2.0 - 2.0
    };
    let across = drift.min(half_key);
    // 每个方向、每个距离都重开一台，上一次敲进去的字母不会串到下一次
    for (dx, dy) in [
        (0.0, 1.0),
        (0.0, 3.0),
        (-6.0, 0.0),
        (0.0, 10.0),
        (across, 0.0),
        (-across, 0.0),
        (0.0, -drift),
        (0.0, drift),
    ] {
        let Some(mut session) = ready() else {
            return;
        };
        drag(&mut session, KeyId::Letter('q'), dx, dy);
        assert_eq!(
            preedit(&session).as_deref(),
            Some("q"),
            "只挪 ({dx}, {dy}) 像素（不够阈值），这一下该还是字母 q"
        );
        assert_eq!(session.take_commit(), None, "不该打出角标");
    }
}

/// **拇指快敲落地时滚一下，不该蹦出角标符号**——用户报的「快打误打符号」。
///
/// 阈值原先定在 12 点，而快敲时手指随手滚一下正好够得着，于是正常打字时不时冒出个 `1`、`@`。
/// 这条把**旧阈值那么大的位移**钉死在「还是字母」上：谁把 [`SWIPE`](crate::keyboard::SWIPE)
/// 调回去，这里先炸。四个方向都试——哪个方向滚回来都该还是字母。
#[test]
fn a_drift_the_size_of_the_old_threshold_no_longer_types_a_symbol() {
    // 改小之前那个阈值（点 → 像素）
    let old = 12.0 * DENSITY;
    for (dx, dy) in [(0.0, 1.0), (1.0, 0.0), (0.7, 0.7), (-0.7, 0.7), (0.0, -1.0)] {
        let Some(mut session) = ready() else {
            return;
        };
        drag(&mut session, KeyId::Letter('q'), dx * old, dy * old);
        assert_eq!(
            preedit(&session).as_deref(),
            Some("q"),
            "往 ({dx}, {dy}) 挪 {old} 像素（旧阈值那么大）该还是字母 q"
        );
        assert_eq!(session.take_commit(), None, "不该打出角标");
    }
}

/// 没有角标的键（数字页、符号页、功能键）往下滑，仍按普通点击算。
///
/// 那些键的键帽上写的就是它自己，没有第二层含义可给。
#[test]
fn a_key_without_a_hint_treats_the_swipe_as_a_plain_tap() {
    let Some(mut session) = ready() else {
        return;
    };

    tap_key(&mut session, KeyId::Panel(Panel::Digits));
    swipe_down(&mut session, KeyId::Literal('7'), swipe_distance());

    assert_eq!(
        session.take_commit().as_deref(),
        Some("7"),
        "没角标的键下滑该还是它自己"
    );
}

/// 拼音里还剩几个字母。
///
/// 数**字母**而不是数长度：拼音变短时引擎会把失去意义的隔音符号 `'` 一起收掉
/// （`ni'h` → `nih` 是少两个字符），字符数不是退格真正删的东西。
fn pinyin_letters(session: &Session) -> usize {
    preedit(session).map_or(0, |text| {
        text.chars().filter(char::is_ascii_alphabetic).count()
    })
}

/// 按住退格不放，壳的计时器到点就问一次 `repeat`——每问一次该少一个字母。
///
/// 计时器在壳那边，所以这里模拟的是「手指按住不动、壳一直问」。
#[test]
fn holding_backspace_repeats() {
    let Some(mut session) = ready() else {
        return;
    };
    type_text(&mut session, "nihao");
    let full = pinyin_letters(&session);
    assert_eq!(
        full,
        5,
        "试不出连删：{}",
        preedit(&session).unwrap_or_default()
    );

    let (x, y) = key_centre(&session, KeyId::Backspace);
    session.touch(MotionAction::Down, POINTER, x, y);

    for round in 1..=3 {
        session.repeat(POINTER);
        assert_eq!(
            pinyin_letters(&session),
            full - round,
            "第 {round} 次连发该少一个字母"
        );
    }

    // 连发过的，松手不该再补一下——不然按住删一串、抬手总会多退一格
    session.touch(MotionAction::Up, POINTER, x, y);
    assert_eq!(
        pinyin_letters(&session),
        full - 3,
        "连发过的手指，抬起不该再补一个"
    );

    // 松手之后计时器还在（壳那边按键与计时器不同步），更不该再删
    session.repeat(POINTER);
    assert_eq!(pinyin_letters(&session), full - 3, "手指已经松了，不该再删");
}

/// 只有退格连发。按住别的键，壳照样到点就问，但不该有任何反应——
/// 字母键按住该出的是角标（往下滑那条路），不是连发。
#[test]
fn only_backspace_repeats() {
    for key in [KeyId::Letter('k'), KeyId::Space, KeyId::Mode, KeyId::Enter] {
        let Some(mut session) = ready() else {
            return;
        };
        let (x, y) = key_centre(&session, key);
        session.touch(MotionAction::Down, POINTER, x, y);
        for _ in 0..5 {
            session.repeat(POINTER);
        }
        assert_eq!(
            preedit(&session),
            None,
            "{key:?} 按住不该连发，更不该往拼音里塞东西"
        );
        assert_eq!(session.take_commit(), None, "{key:?} 按住不该上屏任何东西");
    }
}

/// 手指早就松了、壳的计时器才到点（按键与计时器不同步），不该删任何东西。
#[test]
fn repeat_does_nothing_when_no_finger_is_down() {
    let Some(mut session) = ready() else {
        return;
    };
    type_text(&mut session, "nihao");
    let before = preedit(&session);

    session.repeat(POINTER);

    assert_eq!(preedit(&session), before, "没手指按着，连发该什么也不做");
}

/// 按住键出预览气泡、松手就收；气泡**水平正对着那个键、在键的上方**。
#[test]
fn the_popup_follows_the_pressed_key() {
    let Some(mut session) = ready() else {
        return;
    };
    let (x, y) = key_centre(&session, KeyId::Letter('n'));

    assert!(session.popup_surface().is_empty(), "没按键时不该有气泡");
    assert!(session.popup_origin().is_none(), "没按键时没有摆放位置");

    session.touch(MotionAction::Down, POINTER, x, y);

    let bytes = session.popup_surface();
    assert!(bytes.len() > 8, "按住键该出气泡");
    let (bitmap_w, _) = (
        u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as f32,
        u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as f32,
    );
    let (px, py) = session.popup_origin().expect("该有摆放位置");
    let (kx, ky, kw, _) = key_rect(&session, KeyId::Letter('n'));

    assert!(
        (px + bitmap_w / 2.0 - (kx + kw / 2.0)).abs() < 1.0,
        "气泡该正对着键：气泡中心 {}，键中心 {}",
        px + bitmap_w / 2.0,
        kx + kw / 2.0
    );
    assert!(py < ky, "气泡该在键的上方：气泡顶 {py}，键顶 {ky}");

    session.touch(MotionAction::Up, POINTER, x, y);
    assert!(session.popup_surface().is_empty(), "松手了气泡就该收");
    assert!(session.popup_origin().is_none());
}

/// 边上的键（`⌫`、`回车`）气泡比键还宽，**不许探出屏幕**。
#[test]
fn the_popup_stays_on_screen_at_the_edges() {
    let Some(mut session) = ready() else {
        return;
    };
    let view_width = WIDTH * DENSITY;

    for id in [KeyId::Backspace, KeyId::Panel(Panel::Symbols)] {
        let (x, y) = key_centre(&session, id);
        session.touch(MotionAction::Down, POINTER, x, y);

        let bytes = session.popup_surface();
        assert!(bytes.len() > 8, "{id:?} 该出气泡");
        let bitmap_w = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as f32;
        let (px, _) = session.popup_origin().expect("该有摆放位置");

        assert!(px >= 0.0, "{id:?} 的气泡左边探出屏幕了：{px}");
        assert!(
            px + bitmap_w <= view_width,
            "{id:?} 的气泡右边探出屏幕了：{} > {view_width}",
            px + bitmap_w
        );

        session.touch(MotionAction::Up, POINTER, x, y);
    }
}

/// 在空格上横滑到 `dx` 处按着不动，敲 `ticks` 拍，返回这几拍总共走了几格。
///
/// **不算越过死区那一格**（那是拖动一开始就兑现的），只看「按着不动」的持续速度。
fn cursor_steps_over(dx: f32, ticks: usize) -> Option<isize> {
    let mut session = ready()?;
    let (x, y) = key_centre(&session, KeyId::Space);
    session.touch(MotionAction::Down, POINTER, x, y);
    session.touch(MotionAction::Move, POINTER, x + dx, y);
    session.take_commands();
    let mut total = 0;
    for _ in 0..ticks {
        session.cursor_tick(POINTER);
        total += session.take_commands().len() as isize;
    }
    Some(total)
}

/// 空格上横滑**拖动当中就走**，不用等松手。
///
/// 松手才走的话，手指得先盲拖一段、再抬起来看结果，没法一边看一边调——
/// 用户报的就是这个。
#[test]
fn dragging_on_the_space_bar_moves_the_cursor_right_away() {
    let Some(mut session) = ready() else {
        return;
    };
    let (x, y) = key_centre(&session, KeyId::Space);
    let dead = crate::keyboard::CURSOR_DEAD_ZONE * DENSITY;

    session.touch(MotionAction::Down, POINTER, x, y);
    session.touch(MotionAction::Move, POINTER, x + dead + 10.0, y);

    assert_eq!(
        session.take_commands(),
        vec![Command::MoveRight.code()],
        "刚越过死区就该先走一格，不用等松手"
    );

    // 往左同理
    session.touch(MotionAction::Down, POINTER, x, y);
    session.touch(MotionAction::Move, POINTER, x - dead - 10.0, y);
    assert_eq!(session.take_commands(), vec![Command::MoveLeft.code()]);
}

/// 手指按着不动，光标会**一拍一拍地接着走**（壳每 50ms 敲一拍）。
#[test]
fn holding_the_drag_keeps_the_cursor_moving() {
    let Some(total) = cursor_steps_over(80.0, 10) else {
        return;
    };
    assert!(total >= 5, "按着不动十拍该走好几格，实际 {total}");
}

/// **拖得越远走得越快**——「离按下那点多少像素，走得一格比一格快」。
#[test]
fn the_further_you_drag_the_faster_it_goes() {
    let Some(near) = cursor_steps_over(12.0, 20) else {
        return;
    };
    let Some(far) = cursor_steps_over(200.0, 20) else {
        return;
    };
    assert!(
        far > near,
        "同样二十拍，拖得远该走得多：近处 {near} 格、远处 {far} 格"
    );
}

/// 死区以内不动：那还是「按空格」，上屏高亮候选。
#[test]
fn a_small_drag_on_the_space_bar_is_still_a_space() {
    let Some(mut session) = ready() else {
        return;
    };
    type_text(&mut session, "nihao");
    let first = drawn(&session).first().map(|text| (*text).to_owned());
    let (x, y) = key_centre(&session, KeyId::Space);
    let dead = crate::keyboard::CURSOR_DEAD_ZONE * DENSITY;

    session.touch(MotionAction::Down, POINTER, x, y);
    session.touch(MotionAction::Move, POINTER, x + dead - 2.0, y);
    for _ in 0..10 {
        session.cursor_tick(POINTER);
    }
    session.touch(MotionAction::Up, POINTER, x + dead - 2.0, y);

    assert_eq!(
        session.take_commands(),
        Vec::<i32>::new(),
        "死区以内不该移光标"
    );
    assert_eq!(session.take_commit(), first, "该上屏高亮那个候选");
}

/// 空格上滑出键外照样走——空格键宽，划着划着就出去了，
/// 那不是「取消这一下」，是这个手势本身就该兑现。
#[test]
fn moving_the_cursor_survives_leaving_the_space_bar() {
    let Some(mut session) = ready() else {
        return;
    };
    let (x, y) = key_centre(&session, KeyId::Space);
    let dead = crate::keyboard::CURSOR_DEAD_ZONE * DENSITY;

    session.touch(MotionAction::Down, POINTER, x, y);
    // 一路滑到空格右边的「。」上
    session.touch(MotionAction::Move, POINTER, x + dead + 120.0, y);
    session.cursor_tick(POINTER);

    assert!(!session.take_commands().is_empty(), "滑出空格键外也该照走");
}

/// **组句当中不动光标**：那会儿输入框里是我们的拼音，挪光标该挪拼音里的位置，
/// 引擎还没有这个能力——让应用去挪只会把组字区搅乱。
#[test]
fn the_cursor_does_not_move_while_composing() {
    let Some(mut session) = ready() else {
        return;
    };
    type_text(&mut session, "nihao");
    let (x, y) = key_centre(&session, KeyId::Space);
    let dead = crate::keyboard::CURSOR_DEAD_ZONE * DENSITY;

    session.touch(MotionAction::Down, POINTER, x, y);
    session.touch(MotionAction::Move, POINTER, x + dead + 60.0, y);
    for _ in 0..10 {
        session.cursor_tick(POINTER);
    }

    assert_eq!(
        session.take_commands(),
        Vec::<i32>::new(),
        "还在组句，不该发移光标"
    );
    assert_eq!(
        preedit(&session).as_deref(),
        Some("ni'hao"),
        "拼音该原样留着"
    );
}

/// **横屏的键盘矮一截**——横屏竖向空间少，还用竖屏那个高度会占掉半个屏幕。
#[test]
fn the_keyboard_is_shorter_in_landscape() {
    let Some(mut session) = ready() else {
        return;
    };

    let portrait = session.configure(WIDTH, PORTRAIT_HEIGHT, DENSITY, 0.0, false, false);
    let landscape = session.configure(WIDTH, LANDSCAPE_HEIGHT, DENSITY, 0.0, false, true);

    assert!(
        landscape < portrait,
        "横屏整块输入视图该矮一些：竖屏 {portrait}、横屏 {landscape}"
    );
    assert!(
        portrait - landscape >= 20.0,
        "矮得太少了，只差 {} 点",
        portrait - landscape
    );
}

/// **大屏手机的键盘更高**——高度写死一个值的话，6.7 寸的机器上就显得矮（用户报的）。
#[test]
fn a_bigger_screen_gets_a_taller_keyboard() {
    let Some(mut session) = ready() else {
        return;
    };

    let small = session.configure(WIDTH, 640.0, DENSITY, 0.0, false, false);
    let typical = session.configure(WIDTH, 800.0, DENSITY, 0.0, false, false);
    let big = session.configure(WIDTH, 926.0, DENSITY, 0.0, false, false);

    assert!(
        small <= typical && typical < big,
        "屏幕越大键盘该越高：640 点 → {small}、800 点 → {typical}、926 点 → {big}"
    );
}

/// **平板不许长出一块巨无霸键盘**——上限就是为它设的。
///
/// 原先高度用固定点数而不用百分比，怕的正是「平板横屏八百点高，按百分比算出一块巨无霸」。
/// 改成按屏幕比例之后，这条守着当初那个决定没被丢掉。
#[test]
fn a_tablet_does_not_get_a_giant_keyboard() {
    let Some(mut session) = ready() else {
        return;
    };

    // 量的是**键盘本体**（上面那条细的齿轮条不算在内）
    session.configure(WIDTH, 1200.0, DENSITY, 0.0, false, false);
    let portrait = session.keyboard_height();
    assert!(
        portrait <= 300.0 + 0.01,
        "竖屏上限该夹住：1200 点高的屏幕给到了 {portrait}"
    );

    session.configure(WIDTH, 800.0, DENSITY, 0.0, false, true);
    let landscape = session.keyboard_height();
    assert!(
        landscape <= 260.0 + 0.01,
        "横屏上限该夹住：800 点高的屏幕给到了 {landscape}"
    );
}

/// ⌫ 上**往上滑**、松手 → 把光标前面整段清掉。
#[test]
fn swiping_up_on_backspace_clears_to_the_start() {
    let Some(mut session) = ready() else {
        return;
    };
    let (x, y) = key_centre(&session, KeyId::Backspace);
    let up = crate::keyboard::SWIPE * DENSITY * 1.5;

    session.touch(MotionAction::Down, POINTER, x, y);
    session.touch(MotionAction::Move, POINTER, x, y - up);

    assert_eq!(
        session.take_commands(),
        Vec::<i32>::new(),
        "滑上去只是「预备」，松手之前不该清"
    );

    session.touch(MotionAction::Up, POINTER, x, y - up);
    assert_eq!(
        session.take_commands(),
        vec![Command::ClearToStart.code()],
        "松手才清"
    );
}

/// ⌫ 上只挪一点点、或者往别的方向滑，都还是「按一下退格」。
#[test]
fn a_plain_backspace_is_still_one_backspace() {
    for (dx, dy) in [(0.0, -5.0), (0.0, 5.0), (-5.0, 0.0), (5.0, 0.0)] {
        let Some(mut session) = ready() else {
            return;
        };
        let (x, y) = key_centre(&session, KeyId::Backspace);

        session.touch(MotionAction::Down, POINTER, x, y);
        session.touch(MotionAction::Move, POINTER, x + dx, y + dy);
        session.touch(MotionAction::Up, POINTER, x + dx, y + dy);

        assert_eq!(
            session.take_commands(),
            vec![Command::Backspace.code()],
            "只挪 ({dx}, {dy}) 该还是普通退格"
        );
    }
}

/// 组句当中不做这个手势——那会儿要清的是拼音，引擎那边一条命令的事，
/// 让应用去删只会把组字区搅乱。
#[test]
fn clearing_does_not_apply_while_composing() {
    let Some(mut session) = ready() else {
        return;
    };
    type_text(&mut session, "nihao");
    let (x, y) = key_centre(&session, KeyId::Backspace);
    let up = crate::keyboard::SWIPE * DENSITY * 1.5;

    session.touch(MotionAction::Down, POINTER, x, y);
    session.touch(MotionAction::Move, POINTER, x, y - up);
    session.touch(MotionAction::Up, POINTER, x, y - up);

    assert_eq!(
        session.take_commands(),
        Vec::<i32>::new(),
        "还在组句，不该清空"
    );
    assert_eq!(
        preedit(&session).as_deref(),
        Some("ni'hao"),
        "拼音该原样留着"
    );
}

/// 滑上去之后连发不再删——**手势一开，那个键就不算「按住」了**。
///
/// 不排掉的话，按着不放（连发开始）再往上滑，松手之前连发就已经把字删了一串。
#[test]
fn clearing_suppresses_the_backspace_repeat() {
    let Some(mut session) = ready() else {
        return;
    };
    let (x, y) = key_centre(&session, KeyId::Backspace);
    let up = crate::keyboard::SWIPE * DENSITY * 1.5;

    session.touch(MotionAction::Down, POINTER, x, y);
    session.touch(MotionAction::Move, POINTER, x, y - up);
    session.take_commands();

    // 壳那边按够 400ms 会开始敲连发
    for _ in 0..10 {
        session.repeat(POINTER);
    }

    assert_eq!(
        session.take_commands(),
        Vec::<i32>::new(),
        "已经滑上去了，连发不该再删"
    );
}

/// ⌫ 滑上去之后**气泡要改口**——退格图标说不出「松手会怎样」。
#[test]
fn the_backspace_popup_warns_before_clearing() {
    let Some(mut session) = ready() else {
        return;
    };
    let (x, y) = key_centre(&session, KeyId::Backspace);
    let up = crate::keyboard::SWIPE * DENSITY * 1.5;

    session.touch(MotionAction::Down, POINTER, x, y);
    let plain = session.popup_surface();
    assert!(plain.len() > 8, "按住 ⌫ 该出气泡");

    session.touch(MotionAction::Move, POINTER, x, y - up);
    let warned = session.popup_surface();

    assert!(warned.len() > 8, "滑上去也该有气泡");
    assert_ne!(plain, warned, "滑上去之后气泡该改口说「松手清空」");
    assert!(warned.len() > plain.len(), "提示是句话，位图该比一个图标大");
}

/// **候选条上按住不放不做任何事**——长按就是「慢慢点一下」。
///
/// 原先是「长按候选 = 删词」（K8），2026-09-20 用户说这功能没用，摘掉了：
/// 删词走的是 `Engine::forget`，而引擎缺省那个 `NoLearner` 什么都删不动，
/// 纯词库词更是本来就没学习记录可清——接上 `Learner` 才有意义，见
/// `docs/plan/android-keyboard.md` 的 K8 与 K8+。
///
/// 这条守着两件事：心跳一直敲（壳每 50ms 一次）时**不删任何东西**，
/// 以及**松手仍然照常选中那个候选**——长按不该变成一个「按了没反应」的黑洞。
#[test]
fn holding_a_candidate_does_nothing_but_still_selects_on_release() {
    let Some(mut session) = ready() else {
        return;
    };
    type_text(&mut session, "nihao");
    let first = drawn(&session).first().map(|text| (*text).to_owned());
    let (x, y) = bar_centre(&session, BarHitId::Candidate(0));

    session.touch(MotionAction::Down, POINTER, x, y);
    for _ in 0..10 {
        session.repeat(POINTER);
    }
    session.bar_surface();

    assert_eq!(
        drawn(&session).first().map(|text| (*text).to_owned()),
        first,
        "按住不放不该动候选"
    );

    session.touch(MotionAction::Up, POINTER, x, y);

    assert_eq!(session.take_commit(), first, "松手该照常选中按住的那个候选");
}

/// 空格没有字可显示，按住也不弹——弹一个空框子只是晃眼。
#[test]
fn the_space_key_has_no_popup() {
    let Some(mut session) = ready() else {
        return;
    };
    let (x, y) = key_centre(&session, KeyId::Space);

    session.touch(MotionAction::Down, POINTER, x, y);

    assert!(
        session.popup_surface().is_empty(),
        "空格弹气泡没东西可看，不该弹"
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
    session.touch(MotionAction::PointerUp, 0, nx, ny); // A 抬起 → 上屏 n，候选条顶出来

    // 候选条一顶出来**视图就长高了**，同一根手指在视图里的 y 也就大了整整一个候选条的高度。
    // 真机上安卓自己会这么报（每一拍都是当前坐标），测试里得照着模拟——
    // 拿按下时那对旧坐标去抬，就打到别的键上了
    let (_, iy) = key_centre(&session, KeyId::Letter('i'));
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
    session.touch(MotionAction::PointerUp, 1, ax, ay); // 后按下的先抬 → 候选条顶出来
    // 视图长高了，同一根手指的 y 跟着大一个候选条的高度（见上一个用例的注释）
    let (nx, ny) = key_centre(&session, KeyId::Letter('n'));
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

    // 字母页**直接**进符号页（不必先绕数字页）
    tap_key(&mut session, KeyId::Panel(Panel::Symbols));
    tap_key(&mut session, KeyId::Literal('!'));
    // 符号页的「返回」回字母页
    tap_key(&mut session, KeyId::Panel(Panel::Letters));

    tap_key(&mut session, KeyId::Panel(Panel::Digits));
    tap_key(&mut session, KeyId::Literal('1'));
    // 数字页的「返回」也在「中」原来那个位置
    tap_key(&mut session, KeyId::Panel(Panel::Letters));
    tap_key(&mut session, KeyId::Letter('n'));

    assert_eq!(
        session.take_commit().as_deref(),
        Some("！1"),
        "符号页与数字页打出来的该一前一后都在"
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

/// **长按字母键弹出那排选项，默认（没滑就松手）打的是符号。**
///
/// 2026-09-21 定的：取代原先「在键上往下滑取角标」。那个手势真机上快打仍会误蹦符号，
/// 换成长按——快打按不到 400ms，误触面直接归零。
/// 那排是 **大写 / 符号 / 小写**，默认停在中间那个。
#[test]
fn long_pressing_a_letter_key_types_its_symbol_by_default() {
    let Some(mut session) = ready() else {
        return;
    };
    let (x, y) = key_centre(&session, KeyId::Letter('q'));

    session.touch(MotionAction::Down, POINTER, x, y);
    // 壳按够 400ms 来问一次——这一下开的不是连发，是那排选项
    session.repeat(POINTER);
    session.touch(MotionAction::Up, POINTER, x, y);

    assert_eq!(
        session.take_commit().as_deref(),
        Some("1"),
        "默认选中中间那个：q 的符号是 1"
    );
}

/// 那排里**往左滑选大写、往右滑选小写**；滑出去的距离要过 `Chooser::STEP`。
#[test]
fn sliding_in_the_choice_row_picks_upper_or_lower() {
    let far = 60.0;

    // 往左 → 大写：**直接打出一个大写字母**，不进拼音缓冲区
    let Some(mut session) = ready() else {
        return;
    };
    let (x, y) = key_centre(&session, KeyId::Letter('q'));
    session.touch(MotionAction::Down, POINTER, x, y);
    session.repeat(POINTER);
    session.touch(MotionAction::Move, POINTER, x - far, y);
    session.touch(MotionAction::Up, POINTER, x - far, y);
    assert_eq!(
        session.take_commit().as_deref(),
        Some("Q"),
        "往左滑该打成大写 Q"
    );
    assert_eq!(preedit(&session), None, "大写是直出的，不该进拼音");

    // 往右 → 小写：与直接点一下那个键一样，进拼音缓冲区
    let Some(mut session) = ready() else {
        return;
    };
    let (x, y) = key_centre(&session, KeyId::Letter('q'));
    session.touch(MotionAction::Down, POINTER, x, y);
    session.repeat(POINTER);
    session.touch(MotionAction::Move, POINTER, x + far, y);
    session.touch(MotionAction::Up, POINTER, x + far, y);
    assert_eq!(
        preedit(&session).as_deref(),
        Some("q"),
        "往右滑该是小写 q，照常进拼音"
    );
}

/// **键上滑动不再出符号**——手势整个撤了，符号改由长按那排给。
#[test]
fn swiping_on_a_letter_key_no_longer_types_a_symbol() {
    let Some(mut session) = ready() else {
        return;
    };
    // 往下滑一大截（原先这就是「取角标」）
    drag(&mut session, KeyId::Letter('q'), 0.0, 40.0 * DENSITY);

    assert_eq!(session.take_commit(), None, "下滑不该再吐符号");
    assert_eq!(preedit(&session), None, "手指滑出键外，这一下也不算点了 q");
}

/// 长按**不带角标的键**不开那排：⇧ 按住还是 ⇧，不弹东西。
#[test]
fn long_pressing_a_key_without_a_hint_opens_nothing() {
    let Some(mut session) = ready() else {
        return;
    };
    let (x, y) = key_centre(&session, KeyId::Shift);

    session.touch(MotionAction::Down, POINTER, x, y);
    for _ in 0..5 {
        session.repeat(POINTER);
    }
    session.touch(MotionAction::Up, POINTER, x, y);

    assert_eq!(
        session.take_commit(),
        None,
        "⇧ 按住不该弹出那排、也不该打字"
    );
    assert!(!session.english(), "Shift 那一档也不该被长按弄乱");
}

/// 齿轮开 / 收工具页：同一个按钮管开也管收（仿搜狗那个 S 的手感）。
#[test]
fn the_gear_toggles_the_tools_page() {
    let Some(mut session) = ready() else {
        return;
    };

    tap_bar(&mut session, BarHitId::Tools);
    assert_eq!(session.panel, Panel::Tools, "点齿轮该进工具页");

    tap_bar(&mut session, BarHitId::Tools);
    assert_eq!(session.panel, Panel::Letters, "再点一下该收回字母页");
}

/// 工具页那一行「剪贴板」进剪贴板页；页里的「返回」回字母页。
#[test]
fn the_tools_page_opens_the_clipboard() {
    let Some(mut session) = ready() else {
        return;
    };

    tap_bar(&mut session, BarHitId::Tools);
    tap_key(&mut session, KeyId::Tool(0));
    assert_eq!(session.panel, Panel::Clipboard, "该进剪贴板页");

    tap_key(&mut session, KeyId::Panel(Panel::Letters));
    assert_eq!(session.panel, Panel::Letters, "返回该回字母页");
}

/// 复制的文本一条条记下来：**最新的在最前**，同一条不重复记。
#[test]
fn the_clipboard_keeps_the_newest_first() {
    let Some(mut session) = ready() else {
        return;
    };

    session.note_clipboard("第一段");
    session.note_clipboard("第二段");
    assert_eq!(session.clipboard, ["第二段", "第一段"]);

    // 再复制一次「第一段」：不新记一条，把它挪到最前
    session.note_clipboard("第一段");
    assert_eq!(
        session.clipboard,
        ["第一段", "第二段"],
        "同一条只留一条，而且挪到最前"
    );

    // 空白不当一条记录
    let before = session.clipboard.len();
    session.note_clipboard("   ");
    assert_eq!(session.clipboard.len(), before, "空白不该记");
}

/// 记够上限就丢最旧的。
#[test]
fn the_clipboard_forgets_the_oldest() {
    let Some(mut session) = ready() else {
        return;
    };
    for index in 0..60 {
        session.note_clipboard(&format!("第 {index} 条"));
    }

    assert_eq!(session.clipboard.len(), 50, "最多留 50 条");
    assert_eq!(session.clipboard[0], "第 59 条", "最新的在最前");
    assert_eq!(session.clipboard[49], "第 10 条", "最旧的那十条被挤掉了");
}

/// 点一条记录就把那段文本交给壳上屏（**插在光标处**），然后收回字母页。
#[test]
fn tapping_an_entry_pastes_it() {
    let Some(mut session) = ready() else {
        return;
    };
    session.note_clipboard("粘这段");

    open_clipboard(&mut session);
    tap_key(&mut session, KeyId::Clipboard(0));

    assert_eq!(
        session.take_commit().as_deref(),
        Some("粘这段"),
        "点一条该把它交出去上屏"
    );
    assert_eq!(session.panel, Panel::Letters, "粘完收回字母页，接着打字");
}

/// 空列表时点格子什么也不该发生。
#[test]
fn tapping_an_empty_cell_does_nothing() {
    let Some(mut session) = ready() else {
        return;
    };

    open_clipboard(&mut session);
    tap_key(&mut session, KeyId::Clipboard(0));

    assert_eq!(session.take_commit(), None, "一条都没有，点格子不该上屏");
    assert_eq!(session.panel, Panel::Clipboard, "也不该乱切页");
}

/// 在一条记录上往左滑、松手 → 删掉那条，别的还在。
#[test]
fn swiping_an_entry_left_deletes_it() {
    let Some(mut session) = ready() else {
        return;
    };
    session.note_clipboard("旧的");
    session.note_clipboard("新的");
    open_clipboard(&mut session);

    // 最新在最前：第一格是「新的」
    swipe_left(&mut session, KeyId::Clipboard(0));

    assert_eq!(session.clipboard, ["旧的"], "往左滑过就该删掉那一条");
    assert_eq!(session.take_commit(), None, "删不是上屏");
}

/// 光按一下不滑，不算删——那是「点这条」，会上屏。
#[test]
fn a_tap_on_an_entry_does_not_delete_it() {
    let Some(mut session) = ready() else {
        return;
    };
    session.note_clipboard("点一下");

    open_clipboard(&mut session);
    tap_key(&mut session, KeyId::Clipboard(0));

    assert_eq!(session.clipboard, ["点一下"], "点一下不是删");
    assert_eq!(session.take_commit().as_deref(), Some("点一下"));
}

/// 往左滑过一半**又拖回原位**再松手：不算删——滑错了要能反悔。
///
/// 反悔之后也不该当成「点了一下」——在剪贴板那格上点一下是**粘出去**，
/// 后悔的人不会想粘，所以这一下什么也不该发生。
#[test]
fn swiping_back_cancels_the_delete() {
    let Some(mut session) = ready() else {
        return;
    };
    session.note_clipboard("别删我");
    open_clipboard(&mut session);

    let (x, y) = key_centre(&session, KeyId::Clipboard(0));
    let far = 16.0 * DENSITY + 20.0;

    session.touch(MotionAction::Down, POINTER, x, y);
    session.touch(MotionAction::Move, POINTER, x - far, y);
    // 拖回按下那一点（手指没离开这格）
    session.touch(MotionAction::Move, POINTER, x, y);
    session.touch(MotionAction::Up, POINTER, x, y);
    session.keyboard_surface();

    assert_eq!(session.clipboard, ["别删我"], "拖回来了就不该删");
    assert_eq!(session.take_commit(), None, "反悔之后也不该顺手粘出去");
    assert_eq!(session.panel, Panel::Clipboard, "还留在这页");
}

/// ⌫ 上往上滑过又滑回去、松手：不清——与剪贴板那条一样能反悔。
#[test]
fn sliding_back_cancels_the_clear() {
    let Some(mut session) = ready() else {
        return;
    };
    let (x, y) = key_centre(&session, KeyId::Backspace);
    let up = crate::keyboard::SWIPE * DENSITY * 1.5;

    session.touch(MotionAction::Down, POINTER, x, y);
    session.touch(MotionAction::Move, POINTER, x, y - up);
    session.touch(MotionAction::Move, POINTER, x, y);
    session.touch(MotionAction::Up, POINTER, x, y);

    assert_eq!(
        session.take_commands(),
        Vec::<i32>::new(),
        "滑回来了就不该清，也不该退格"
    );
}

/// 「清空」把整份历史清掉。
#[test]
fn the_clear_key_empties_the_clipboard() {
    let Some(mut session) = ready() else {
        return;
    };
    session.note_clipboard("一条");
    session.note_clipboard("两条");
    open_clipboard(&mut session);

    tap_key(&mut session, KeyId::ClipboardClear);

    assert!(session.clipboard.is_empty(), "该一条不剩");
    assert_eq!(session.take_commit(), None, "清空不上屏任何东西");
}

/// 剪贴板列表**上下跟手滚**（2026-09-21 取代了原先的 `‹ ›` 翻页）：
/// 往上拖看后面的条目，拖到头就停住、也滚不动了。
#[test]
fn the_clipboard_list_scrolls_under_the_finger() {
    let Some(mut session) = ready() else {
        return;
    };
    for index in 0..8 {
        session.note_clipboard(&format!("第 {index} 条"));
    }
    open_clipboard(&mut session);

    assert_eq!(session.clipboard_scroll, 0.0, "进来从头看起");
    assert_eq!(session.clipboard_first(), 0);
    assert_eq!(session.frame.footer, None, "剪贴板没有页码了");

    // 往上拖一格多一点：看后面的条目
    let pitch = session.clipboard_pitch();
    scroll_up(&mut session, pitch * DENSITY * 1.5);
    assert!(
        session.clipboard_scroll > 0.0,
        "往上拖该把列表滚上去，实际 {}",
        session.clipboard_scroll
    );
    assert_eq!(session.clipboard_first(), 1, "滚过一整格就该从第二条起");

    // 一直往上拖：停在「还能滚多远」那儿，不会滚出空白
    for _ in 0..10 {
        scroll_up(&mut session, pitch * DENSITY * 3.0);
    }
    let max = (8 - CLIPBOARD_CELLS) as f32 * pitch;
    assert!(
        (session.clipboard_scroll - max).abs() < 0.01,
        "拖到底该停在 {max}，实际 {}",
        session.clipboard_scroll
    );

    // 再往下拖回去：回到最新的那几条
    for _ in 0..10 {
        scroll_down(&mut session, pitch * DENSITY * 3.0);
    }
    assert_eq!(session.clipboard_scroll, 0.0, "往下拖到头就是最新那条贴顶");
}

/// 拖得再远也滚不出列表末尾：多出来几格就只滚几格。
#[test]
fn a_long_drag_stops_at_the_end_of_the_list() {
    let Some(mut session) = ready() else {
        return;
    };
    for index in 0..6 {
        session.note_clipboard(&format!("第 {index} 条"));
    }
    open_clipboard(&mut session);
    let pitch = session.clipboard_pitch();

    scroll_up(&mut session, pitch * DENSITY * 5.0);

    assert_eq!(
        session.clipboard_scroll, pitch,
        "六条比一屏多一条，只该滚一格"
    );
    assert_eq!(session.clipboard_first(), 1, "滚一格就该从第二条起");
}

/// 滚下去之后**点的是屏幕上那一格对着的那条**，不是整份第一条。
#[test]
fn a_scrolled_cell_pastes_the_entry_under_it() {
    let Some(mut session) = ready() else {
        return;
    };
    for index in 0..8 {
        session.note_clipboard(&format!("第 {index} 条"));
    }
    open_clipboard(&mut session);

    // 往上拖一格多一点：屏幕上第一格现在是整份里的第二条
    let pitch = session.clipboard_pitch();
    scroll_up(&mut session, pitch * DENSITY * 1.2);
    assert_eq!(session.clipboard_first(), 1);

    tap_key(&mut session, KeyId::Clipboard(0));

    // 最新在最前，整份是 [第7, 第6, 第5, 第4, 第3, 第2, 第1, 第0]：
    // 滚过一格之后屏幕上第一格 = 整份第 1 个（从 0 数）= 「第 6 条」
    assert_eq!(
        session.take_commit().as_deref(),
        Some("第 6 条"),
        "滚过一格之后点的该是整份第 1 个，不是第一个"
    );
}

/// 删到不够一屏时滚动量要夹回来，不能停在一段空白上。
#[test]
fn deleting_the_last_entry_pulls_the_list_back() {
    let Some(mut session) = ready() else {
        return;
    };
    // 六条：正好一屏五条 + 多出来一条，能往下滚一格
    for index in 0..6 {
        session.note_clipboard(&format!("第 {index} 条"));
    }
    open_clipboard(&mut session);
    let pitch = session.clipboard_pitch();
    scroll_up(&mut session, pitch * DENSITY * 1.5);
    assert!(session.clipboard_scroll > 0.0, "六条该能滚一格");

    // 滚到底时屏幕上第一条就是最后那条，删掉它就没得滚了
    swipe_left(&mut session, KeyId::Clipboard(0));

    assert_eq!(session.clipboard.len(), 5);
    assert_eq!(session.clipboard_scroll, 0.0, "只剩一屏，滚动量该夹回来");
}
