//! 配置读写桥的测试：设置页要用的那几个入口。

use std::path::{Path, PathBuf};

use qingjian_platform::{Config, VibrationStyle};

use super::{
    DICTS_DIR, config_path, domain_list, read_json, set_array, set_bool, set_int, set_string,
};

/// 造一个临时数据目录。**每个测试用自己的名字**——测试是并行跑的，
/// 共用一个目录会互相删（E4 那轮踩过）。
fn data_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("qingjian-settings-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn json(text: &str) -> serde_json::Value {
    serde_json::from_str(text).expect("该是一段 JSON")
}

/// **设置页会写的每一个键都走一遍往返**。
///
/// `Config::set_value` 不校验键存不存在，拼错一个就会在配置文件里造出个没人读的垃圾键；
/// 这条测试把「壳那边改了键名、这边没跟上」挡在 CI 里（两边都得改才过得去）。
#[test]
fn every_key_the_settings_page_writes_round_trips() {
    let dir = data_dir("roundtrip");
    // 从「文件还没有」开始：写第一笔时会自动落一份模板出来
    assert!(Config::write_template_if_missing(&config_path(&dir)).unwrap());

    assert!(set_string(&dir, "general", "learning_language", "ja").is_empty());
    assert!(set_string(&dir, "keyboard", "vibration", "heavy").is_empty());
    assert!(set_int(&dir, "keyboard", "vibration_ms", 35).is_empty());
    assert!(set_bool(&dir, "fuzzy", "z_zh", true).is_empty());
    assert!(set_bool(&dir, "predict", "enabled", true).is_empty());
    assert!(set_string(&dir, "predict", "base_url", "https://example.com/v1").is_empty());
    assert!(set_string(&dir, "predict", "model", "some-model").is_empty());
    assert!(set_string(&dir, "predict", "api_key", "sk-test").is_empty());
    assert!(set_int(&dir, "predict", "slots", 3).is_empty());
    assert!(set_bool(&dir, "predict", "sentence", false).is_empty());
    assert!(
        set_array(
            &dir,
            "dictionaries",
            "domains",
            &["idioms".to_owned(), "law".to_owned()],
        )
        .is_empty()
    );

    let config = Config::load(&config_path(&dir)).unwrap();
    assert_eq!(config.general.learning_language, "ja");
    assert_eq!(config.keyboard.vibration, VibrationStyle::Heavy);
    assert_eq!(config.keyboard.vibration_ms, 35);
    assert!(config.fuzzy.z_zh);
    assert!(config.predict.enabled);
    assert_eq!(config.predict.base_url, "https://example.com/v1");
    assert_eq!(config.predict.model, "some-model");
    assert_eq!(config.predict.api_key.as_deref(), Some("sk-test"));
    assert_eq!(config.predict.slots, 3);
    assert!(!config.predict.sentence);
    assert_eq!(config.dictionaries.domains, ["idioms", "law"]);
    // 没碰过的键仍是缺省——写一个键不该把别的弄丢
    assert_eq!(config.general.page_size(), 9);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reading_hands_back_an_envelope() {
    let dir = data_dir("read");
    // 文件还没有：按缺省，不是错误
    let envelope = json(&read_json(&dir));
    assert_eq!(envelope["ok"], true);
    assert_eq!(envelope["config"]["general"]["learning_language"], "en");
    assert_eq!(envelope["config"]["keyboard"]["vibration"], "click");

    // 手改坏了：**把错误原样交给壳**，让设置页显示出来
    std::fs::write(config_path(&dir), "[fuzzy\nz_zh = true\n").unwrap();
    let broken = json(&read_json(&dir));
    assert_eq!(broken["ok"], false);
    assert!(
        broken["error"]
            .as_str()
            .is_some_and(|message| !message.is_empty()),
        "错误得说得出话，实际是 {:?}",
        broken["error"]
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// 文件坏了就**拒写、一个字节都不动**——不替用户「修好」（那会把他写的注释和顺序一起抹掉）。
#[test]
fn writing_refuses_a_broken_file_and_leaves_it_alone() {
    let dir = data_dir("broken");
    let path = config_path(&dir);
    let original = "# 我自己写的注释\n[fuzzy\nz_zh = true\n";
    std::fs::write(&path, original).unwrap();

    assert!(!set_bool(&dir, "fuzzy", "z_zh", false).is_empty(), "该报错");
    assert!(
        !set_string(&dir, "general", "learning_language", "ja").is_empty(),
        "该报错"
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn writing_keeps_the_comments_around_the_key() {
    let dir = data_dir("comments");
    let path = config_path(&dir);
    std::fs::write(
        &path,
        "# 头\n[keyboard]\n# 震动那条的说明\nvibration = \"click\"\n",
    )
    .unwrap();

    assert!(set_string(&dir, "keyboard", "vibration", "tick").is_empty());
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("# 头"), "{text}");
    assert!(text.contains("# 震动那条的说明"), "{text}");
    assert!(text.contains("vibration = \"tick\""), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// 领域词库的清单是**从词库文件里读出来的**，壳那边不硬编码那 11 本。
#[test]
fn domain_list_reads_the_names_out_of_the_dictionary_files() {
    let dir = data_dir("domains");
    std::fs::create_dir_all(dir.join(DICTS_DIR)).unwrap();
    // 名字带 `.qj` 但内容是 TSV：`Dictionary::from_path` 按**魔术字节**认容器，不看扩展名
    std::fs::write(
        dir.join(DICTS_DIR).join("medicine.qj"),
        "醛固酮\tquan gu tong\t100\n",
    )
    .unwrap();
    std::fs::write(dir.join(DICTS_DIR).join("notes.txt"), "不是词库\n").unwrap();

    let books = json(&domain_list(&dir));
    assert_eq!(books.as_array().map(Vec::len), Some(1), "只该列出词库文件");
    assert_eq!(books[0]["stem"], "medicine");
    // 裸 TSV 没有元数据，退回文件名
    assert_eq!(books[0]["name"], "medicine");

    // 目录不在（随包资源还没解出来）时是空数组，不是崩
    assert_eq!(domain_list(Path::new("不存在")), "[]");
    let _ = std::fs::remove_dir_all(&dir);
}
