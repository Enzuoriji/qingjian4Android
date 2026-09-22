//! 配置文件的读写桥：给安卓设置页那一侧用的。
//!
//! **设置页只跟 `config.toml` 打交道，不碰会话句柄**——`handle` 是 `Box<Session>` 的裸指针，
//! 输入法服务随时可能在 `onDestroy` 里把它拆掉，设置页拿着就是个野指针。
//! 会话那边读同一份文件（[`crate::session::config`]），两边都从这里拼路径，不各拼各的。
//!
//! 值全部以 JSON 过桥（读一次拿整份，写则逐键），所以壳那边用安卓自带的 `org.json` 就能解，
//! 不必引任何第三方库。

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use qingjian_dictionary::Dictionary;
use qingjian_platform::{Config, ConfigError, extra_dictionaries};
use qingjian_predict::ConnectionTest;
use serde_json::json;

use crate::session::DICTS_DIR;

#[cfg(test)]
mod tests;

/// 正在进行的那次「测试连接」。同一时刻只留一次——再点一次就是重新开始。
///
/// 放静态量而不是会话里：设置页**不碰会话句柄**（那个指针随时可能失效），
/// 探活这件事跟输入法会话本来也没关系。
static CLOUD_TEST: Mutex<Option<ConnectionTest>> = Mutex::new(None);

/// 开始测试云服务连接：**空串表示开始了**，非空是没能开始的原因（比如没填密钥）。
///
/// 读的是**文件里当前那份**配置——用户可能刚在同一个页面上填完密钥，
/// 而这一页没有「保存」按钮，改一项就写一项，所以文件里那份就是最新的。
pub fn cloud_test_start(data_dir: &Path) -> String {
    let config = match Config::load(&config_path(data_dir)) {
        Ok(config) => config,
        Err(error) => return error.to_string(),
    };
    match ConnectionTest::start(&config.predict) {
        Ok(test) => {
            *lock() = Some(test);
            String::new()
        }
        Err(error) => error.to_string(),
    }
}

/// 取测试结果，以 JSON 给壳：`{"done":false}` 是还没回来，`{"done":true,"ok":…,"text":…}` 是结果。
///
/// 取过一次就把这次测试丢掉——壳那边问到了就不必再问。
pub fn cloud_test_poll() -> String {
    let mut guard = lock();
    let Some(report) = guard.as_ref().and_then(ConnectionTest::poll) else {
        return json!({ "done": false }).to_string();
    };
    *guard = None;
    match report {
        Ok(report) => json!({
            "done": true,
            "ok": true,
            "text": format!(
                "连接成功：{} 在 {} 毫秒内回话",
                report.model,
                report.elapsed.as_millis()
            ),
        })
        .to_string(),
        Err(error) => json!({ "done": true, "ok": false, "text": error.to_string() }).to_string(),
    }
}

/// 拿那把锁。**中毒了也接着用**：里面就是个「有没有在测」的状态，
/// 上一次测试 panic 了不该让设置页从此点不动。
fn lock() -> std::sync::MutexGuard<'static, Option<ConnectionTest>> {
    CLOUD_TEST.lock().unwrap_or_else(|error| error.into_inner())
}

/// 配置文件在数据目录里的名字。
pub const CONFIG_FILE: &str = "config.toml";

/// 配置文件路径。**全应用只在这一处拼**——会话与设置页各拼一份迟早会岔开。
pub fn config_path(data_dir: &Path) -> PathBuf {
    data_dir.join(CONFIG_FILE)
}

/// 读整份配置，返回给壳的信封：成功 `{"ok":true,"config":{…}}`，失败 `{"ok":false,"error":"…"}`。
///
/// **缺省值由 serde 填好**，所以壳拿到的那份就是当前生效值，与桌面同源。
/// 读不出来时**把错误原样交给壳**，让设置页显示出来——不替用户「修好」文件，
/// 那会把他的注释和顺序一起抹掉（桌面两壳也是这条规矩）。
pub fn read_json(data_dir: &Path) -> String {
    match Config::load(&config_path(data_dir)) {
        Ok(config) => json!({ "ok": true, "config": config }).to_string(),
        Err(error) => json!({ "ok": false, "error": error.to_string() }).to_string(),
    }
}

/// 把一次写盘的结果变成给壳的话：**空串 = 成功**，非空是错误文案。
///
/// **收结果而不是收闭包**：`ConfigError` 个头不小（带路径与 toml 的报错），
/// 让闭包返回 `Result<(), ConfigError>` 会踩 clippy 的 `result_large_err`。
fn write(result: Result<(), ConfigError>) -> String {
    result
        .err()
        .map_or_else(String::new, |error| error.to_string())
}

/// 写一个开关。
pub fn set_bool(data_dir: &Path, section: &str, key: &str, value: bool) -> String {
    write(Config::set_bool(
        &config_path(data_dir),
        section,
        key,
        value,
    ))
}

/// 写一个整数。
///
/// **值必须按类型分开写**：`set_value` 收的是 `impl Into<toml_edit::Value>`，
/// 把 `false` 当字符串写进去会变成 `"false"`，下次 `Config::load` 直接解析失败。
pub fn set_int(data_dir: &Path, section: &str, key: &str, value: i64) -> String {
    write(Config::set_value(
        &config_path(data_dir),
        section,
        key,
        value,
    ))
}

/// 写一个字符串（学习语言、震动风格这类枚举也走它：它们在 TOML 与 JSON 里的写法
/// 逐字相同，壳不需要映射表）。
pub fn set_string(data_dir: &Path, section: &str, key: &str, value: &str) -> String {
    write(Config::set_value(
        &config_path(data_dir),
        section,
        key,
        value,
    ))
}

/// 写一串字符串（领域词库那种清单）。
pub fn set_array(data_dir: &Path, section: &str, key: &str, values: &[String]) -> String {
    write(Config::set_array(
        &config_path(data_dir),
        section,
        key,
        values,
    ))
}

/// 列出随包的领域词库：JSON 数组，每项 `{"stem":"medicine","name":"医学"}`。
///
/// 中文名从 `.qj` 的元数据里读（`tools/dict-convert` 的 `DOMAIN_NAMES` 写进去的），
/// **不在这儿硬编码一份**——与 E6「有几本挂几本」同一个路数：以后加一本词库，
/// 设置页跟着就有了，两边代码都不用改。读不出元数据的（坏文件）退回文件名。
pub fn domain_list(bundle_dir: &Path) -> String {
    let books: Vec<_> = extra_dictionaries::list(&bundle_dir.join(DICTS_DIR))
        .into_iter()
        .map(|(stem, path)| {
            let name = Dictionary::from_path(&path)
                .ok()
                .and_then(|dictionary| dictionary.metadata().map(|meta| meta.name.clone()))
                .unwrap_or_else(|| stem.clone());
            json!({ "stem": stem, "name": name })
        })
        .collect();
    json!(books).to_string()
}
