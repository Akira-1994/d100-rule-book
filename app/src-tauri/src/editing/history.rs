//! 變更歷史就是 git。
//!
//! `data/errata/` 在 git 裡，歷史已經有了 —— 自己再做一份 journal 只會
//! 多一個會跟 git 不同步的真相來源。

use std::process::Command;

use serde::Serialize;

use super::repo_root;

#[derive(Serialize)]
pub struct ErrataHistory {
    /// 尚未提交的變更（`git diff`）。空字串代表沒有未提交的改動。
    pub uncommitted: String,
    /// 已提交的紀錄（`git log --oneline`）。
    pub commits: Vec<Commit>,
    /// git 不可用時說明原因，而不是讓整頁失敗。
    pub unavailable: Option<String>,
}

#[derive(Serialize)]
pub struct Commit {
    pub hash: String,
    pub subject: String,
    pub date: String,
}

pub fn history() -> ErrataHistory {
    let root = repo_root();
    let errata = "data/errata";

    let diff = match git(&["diff", "--", errata]) {
        Ok(text) => text,
        Err(e) => {
            return ErrataHistory {
                uncommitted: String::new(),
                commits: Vec::new(),
                unavailable: Some(e),
            };
        }
    };

    // 未追蹤的新檔案不會出現在 git diff 裡 —— 新開一張工作表的勘誤檔時
    // 就是這種情況，不列出來會讓人以為改動沒存到。
    let untracked = git(&["ls-files", "--others", "--exclude-standard", "--", errata])
        .unwrap_or_default();

    let mut uncommitted = diff;
    if !untracked.trim().is_empty() {
        uncommitted.push_str("\n# 尚未加入 git 的新檔案：\n");
        for line in untracked.lines().filter(|l| !l.trim().is_empty()) {
            uncommitted.push_str(&format!("#   {line}\n"));
        }
    }

    let log = git(&[
        "log",
        "-20",
        "--date=short",
        "--format=%h\u{1}%s\u{1}%ad",
        "--",
        errata,
    ])
    .unwrap_or_default();

    let commits = log
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|line| {
            let mut parts = line.split('\u{1}');
            Some(Commit {
                hash: parts.next()?.to_string(),
                subject: parts.next()?.to_string(),
                date: parts.next().unwrap_or_default().to_string(),
            })
        })
        .collect();

    let _ = root;
    ErrataHistory {
        uncommitted,
        commits,
        unavailable: None,
    }
}

fn git(args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(repo_root())
        .output()
        .map_err(|e| format!("找不到 git：{e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .replace("\r\n", "\n")
        .to_string())
}
