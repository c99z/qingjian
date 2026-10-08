//! 资源能力枚举：设置界面按它生成可选项——没有释义表的语言不提供启用选项，
//! 没有的模型文件显示「未安装」而不是开关假装能开。

use std::path::{Path, PathBuf};

use qingjian_core::Language;

use crate::extra_dictionaries;

/// 一个被枚举出来的文件或目录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedFile {
    /// 名字（文件名词干或目录名）。
    pub name: String,

    pub path: PathBuf,

    /// 当前是否存在。
    pub exists: bool,
}

/// 一次快照时点上的资源能力。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResourceCapabilities {
    /// 可用的随包领域词库（`data/generated/dicts/` 下，名字是文件名词干）。
    pub bundled_domains: Vec<String>,

    /// 用户词库文件（用户数据目录 `dicts/` 下的 `.qj` / `.tsv`）。
    pub user_dictionaries: Vec<NamedFile>,

    /// 有释义表文件的学习语言（ISO 639-1 代码）。
    pub glossary_languages: Vec<String>,

    /// 本地整句模型的候选位置，按加载优先级排序（与 Server 的 `find_model` 顺序一致）。
    pub model_candidates: Vec<NamedFile>,
}

impl ResourceCapabilities {
    /// `resource_root` 是随包资源根（下有 `data/` 与 `assets/`），`user_dir` 是用户数据目录；
    /// 资源根找不到时随包部分为空，用户部分照常枚举。
    pub fn discover(resource_root: Option<&Path>, user_dir: &Path) -> Self {
        let bundled_domains = resource_root
            .map(|root| extra_dictionaries::list(&root.join("data/generated/dicts")))
            .unwrap_or_default()
            .into_iter()
            .map(|(stem, _)| stem)
            .collect();
        let user_dictionaries = extra_dictionaries::list(&user_dir.join("dicts"))
            .into_iter()
            .map(|(stem, path)| NamedFile {
                name: stem,
                exists: path.is_file(),
                path,
            })
            .collect();
        let glossary_languages = match resource_root {
            Some(root) => [Language::English, Language::Japanese, Language::Spanish]
                .into_iter()
                .filter(|language| {
                    let code = language.code();
                    root.join(format!("data/generated/glossary-{code}.qj"))
                        .is_file()
                        || root
                            .join(format!("assets/glossary/glossary-{code}.tsv"))
                            .is_file()
                })
                .map(|language| language.code().to_owned())
                .collect(),
            None => Vec::new(),
        };
        // 与 apps/linux/server 的 find_model 同一顺序：P2C 优先，同类模型里用户目录优先。
        let mut candidates = vec![
            user_dir.join("models/hanzhang-tongbian"),
            user_dir.join("model-p2c"),
        ];
        if let Some(root) = resource_root {
            candidates.push(root.join("data/models/hanzhang-tongbian"));
        }
        candidates.push(user_dir.join("models/hanzhang-zhiwei"));
        candidates.push(user_dir.join("model"));
        if let Some(root) = resource_root {
            candidates.push(root.join("data/models/hanzhang-zhiwei"));
        }
        let model_candidates = candidates
            .into_iter()
            .map(|path| NamedFile {
                name: path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                exists: path.is_dir(),
                path,
            })
            .collect();
        Self {
            bundled_domains,
            user_dictionaries,
            glossary_languages,
            model_candidates,
        }
    }
}
