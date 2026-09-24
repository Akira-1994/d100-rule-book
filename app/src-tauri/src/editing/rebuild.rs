//! 重跑 `tools/build_db.py`。
//!
//! 這是既有流程，不是新發明的 —— 改完 errata 本來就要重建，只是現在由
//! 應用代勞。

use std::path::Path;
use std::process::Command;

use serde::Serialize;

use super::repo_root;

#[derive(Serialize)]
pub struct RebuildResult {
    pub ok: bool,
    /// 成功時是建置的統計摘要，失敗時是 stderr 原文。
    pub output: String,
}

/// Python 與 PyYAML 在不在。
///
/// 啟動時查一次，讓按鈕在按下去之前就知道自己能不能用 —— 按下去才失敗
/// 是最糟的一種。
pub fn python_available() -> (bool, Option<String>) {
    match python().arg("-c").arg("import yaml").output() {
        Ok(out) if out.status.success() => (true, None),
        Ok(_) => (
            false,
            Some("找到 python 但缺少 PyYAML。請執行 pip install -r tools/requirements.txt".into()),
        ),
        Err(_) => (
            false,
            Some("找不到 python。編輯需要它來重建資料庫。".into()),
        ),
    }
}

pub fn run() -> RebuildResult {
    let root = repo_root();
    let script = root.join("tools").join("build_db.py");
    if !script.is_file() {
        return RebuildResult {
            ok: false,
            output: format!("找不到 {}", script.display()),
        };
    }

    match python().arg(&script).current_dir(&root).output() {
        Ok(out) => {
            let stdout = decode(&out.stdout);
            let stderr = decode(&out.stderr);
            if out.status.success() {
                RebuildResult {
                    ok: true,
                    output: stdout.trim().to_string(),
                }
            } else {
                // 最常見的失敗是勘誤過期（「找不到 <工作表> 第 N 列」），
                // 那要讓人看到原句才知道該去改哪裡。
                let message = if stderr.trim().is_empty() {
                    stdout
                } else {
                    stderr
                };
                RebuildResult {
                    ok: false,
                    output: message.trim().to_string(),
                }
            }
        }
        Err(e) => RebuildResult {
            ok: false,
            output: format!("無法執行 python：{e}"),
        },
    }
}

/// `build_db.py` 的訊息全是中文，而 Windows 的 Python 預設不是 UTF-8 輸出
/// —— 不指定的話拿到的會是亂碼，亂碼的錯誤訊息等於沒有錯誤訊息。
fn python() -> Command {
    let mut cmd = Command::new("python");
    cmd.env("PYTHONIOENCODING", "utf-8");
    cmd
}

fn decode(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).replace("\r\n", "\n")
}

/// 用 PyYAML 確認檔案仍可解析。
///
/// 刻意用 Python 而不是 Rust 的 YAML 程式庫：Python 本來就是這個功能的
/// 硬前置，而且這樣驗的是 `build_db.py` 實際會用的同一個解析器 ——
/// 兩個解析器對邊界情況的看法不一定相同。
pub fn yaml_parses(path: &Path) -> Result<(), String> {
    let script = format!(
        "import sys, yaml; yaml.safe_load(open(r'''{}''', encoding='utf-8'))",
        path.display()
    );
    match python().arg("-c").arg(script).output() {
        Ok(out) if out.status.success() => Ok(()),
        Ok(out) => Err(decode(&out.stderr).trim().to_string()),
        Err(e) => Err(format!("無法執行 python：{e}")),
    }
}
