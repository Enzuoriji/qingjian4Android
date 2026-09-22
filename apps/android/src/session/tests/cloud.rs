//! 云联想：接没接上、什么时候问、**什么时候绝对不问**。
//!
//! 这些用例不会真发请求（配的地址连不上，失败只记日志），要验的是**发不发**这件事。

use std::path::Path;

use super::{DENSITY, PORTRAIT_HEIGHT, WIDTH, config_dir, dictionary, type_text};
use crate::session::Session;

/// 开一个会话，两块面都画一次。`data` 里那份 `config.toml` 就是「用户的配置」。
fn open(dictionary: &Path, data: &Path) -> Session {
    let mut session = Session::open(dictionary, "zh-CN", None, Some(data)).expect("会话该能打开");
    session.configure(WIDTH, PORTRAIT_HEIGHT, DENSITY, 0.0, false, false);
    session.keyboard_surface();
    session.bar_surface();
    session
}

/// 一份「开着云联想、也填了密钥」的配置。
///
/// 地址故意指向一个连不上的端口：这几个用例要验的是**发不发**，不是发得通不通
/// （发不通只会在 worker 里记一条 warn，不影响断言）。
const CLOUD_ON: &str = r#"
[predict]
enabled = true
api_key = "test-key"
base_url = "http://127.0.0.1:9"
"#;

/// 没配云联想时：接了的是「不联想」那个空实现，一次都不问。
#[test]
fn without_a_key_nothing_is_ever_sent() {
    let Some(dictionary) = dictionary() else {
        return;
    };
    let data = config_dir("cloud_off", "[predict]\nenabled = false\n");
    let mut session = open(&dictionary, &data);

    assert!(!session.engine.prediction_enabled(), "没开就不该接上");
    type_text(&mut session, "nihao");
    assert!(!session.predicting, "没接上就不该有请求在飞");
    assert_eq!(session.poll_prediction(), 0);

    let _ = std::fs::remove_dir_all(&data);
}

/// 开着云联想：敲字就问。
#[test]
fn a_configured_cloud_gets_asked() {
    let Some(dictionary) = dictionary() else {
        return;
    };
    let data = config_dir("cloud_on", CLOUD_ON);
    let mut session = open(&dictionary, &data);
    assert!(session.engine.prediction_enabled(), "配了密钥就该接上");

    type_text(&mut session, "nihao");
    assert!(session.predicting, "中文模式敲够字母就该问云");

    let _ = std::fs::remove_dir_all(&data);
}

/// **密码框里一个字节都不发**（2026-09-22 定的隐私底线）。
///
/// 判定在壳那边（`EditorInfo.inputType`），这里守的是**引擎那道闸**：
/// 壳报 `private` 之后，联想一律不发——连已经发起的那一轮也作废。
#[test]
fn a_password_field_never_reaches_the_cloud() {
    let Some(dictionary) = dictionary() else {
        return;
    };
    let data = config_dir("cloud_private", CLOUD_ON);
    let mut session = open(&dictionary, &data);

    type_text(&mut session, "nihao");
    assert!(session.predicting, "先确认正常情况下是会问的");

    // 用户点进了密码框
    session.set_private(true);
    assert!(!session.predicting, "进了密码框，这一轮就该作废");
    assert!(session.sentence.is_none(), "整句补全也该收掉");
    assert!(session.cloud_words.is_empty(), "云端词也该收掉");

    // 再敲：还是不问
    type_text(&mut session, "mima");
    assert!(!session.predicting, "密码框里不该有任何请求");

    let _ = std::fs::remove_dir_all(&data);
}

/// 没拿到整句时点那块地方：什么都不该发生（不能凭空上屏）。
#[test]
fn accepting_without_a_sentence_does_nothing() {
    let Some(dictionary) = dictionary() else {
        return;
    };
    let data = config_dir("cloud_accept", "[predict]\nenabled = false\n");
    let mut session = open(&dictionary, &data);

    type_text(&mut session, "nihao");
    assert!(session.sentence.is_none());
    session.apply(crate::action::Act::AcceptPrediction);
    assert!(
        session.take_commit().is_none(),
        "没有整句可接受时不该上屏任何东西"
    );

    let _ = std::fs::remove_dir_all(&data);
}

/// 整句补全要占的那一行：**云联想开着就留着**，哪怕一个字都还没敲出来。
///
/// 这条守的是「敲一个键不会顶动应用内容」——高度得由会话级的开关定，
/// 不能等结果回来才长高（那一下正打在用户打字的时候）。
#[test]
fn the_bottom_line_is_reserved_as_soon_as_the_cloud_is_on() {
    let Some(dictionary) = dictionary() else {
        return;
    };
    let with_cloud = config_dir("cloud_line_on", CLOUD_ON);
    let without = config_dir("cloud_line_off", "[predict]\nenabled = false\n");

    let mut on = open(&dictionary, &with_cloud);
    let mut off = open(&dictionary, &without);

    // 同样的输入，一个有云一个没有：组句时高度不一样（云那条多留了一行）
    type_text(&mut on, "nihao");
    type_text(&mut off, "nihao");
    assert!(
        on.bar_height() > off.bar_height(),
        "开着云联想该多留一行：{} vs {}",
        on.bar_height(),
        off.bar_height()
    );

    let _ = std::fs::remove_dir_all(&with_cloud);
    let _ = std::fs::remove_dir_all(&without);
}
