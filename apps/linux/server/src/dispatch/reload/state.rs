//! 配置热重载记的状态与数据目录。
use std::path::PathBuf;
use std::time::{Instant, SystemTime};

use qingjian_core::Language;
use qingjian_platform::{Config, DictionariesConfig};

/// 热重载要用的目录：与启动装配用的是同一批，释义表与附加词库才找得到文件。
#[derive(Debug, Clone)]
pub struct ReloadDirs {
    /// 随包资源根（释义表在 `data/generated/` 与 `assets/glossary/` 下）。
    pub resource_root: PathBuf,

    /// 用户数据目录（个人释义表与用户 `dicts/` 的父目录）；没有则换释义表只用随包表。
    pub user_dir: Option<PathBuf>,

    /// 随包领域词库目录。
    pub bundled_dicts: Option<PathBuf>,

    /// 输入日志文件路径；没有用户目录时为 `None`，开关切换只记警告。
    pub input_log: Option<PathBuf>,
}

impl ReloadDirs {
    /// 用户导入词库目录 `<用户目录>/dicts`；启动装配时已创建，这里只读。
    pub(crate) fn user_dicts(&self) -> Option<PathBuf> {
        self.user_dir.as_ref().map(|dir| dir.join("dicts"))
    }
}

/// 热重载状态；`None` 表示没开（单元测试）。
pub(crate) struct ConfigReload {
    /// `config.toml` 路径。
    pub(crate) config_path: PathBuf,

    /// 上次看文件的时间（轮询节流用）。
    pub(crate) last_check: Instant,

    /// 上次看到的 mtime；原子替换后路径的 mtime 变化即视为改动。
    pub(crate) last_mtime: Option<SystemTime>,

    /// 数据目录。
    pub(crate) dirs: ReloadDirs,

    /// 已应用的学习语言（`None` 为关）。
    pub(crate) applied_language: Option<Language>,

    /// 已应用的 `[dictionaries]`。
    pub(crate) applied_dictionaries: DictionariesConfig,

    /// 已应用的 `[model] enabled`。
    pub(crate) applied_model: bool,

    /// 已应用的 `[general] input_log`。
    pub(crate) applied_input_log: bool,

    /// 等组句结束才应用的配置。
    pub(crate) pending: Option<Box<Config>>,

    /// 配置代次，每次成功应用递增。
    pub(crate) generation: u64,

    /// 上次重载 / 解析的错误。
    pub(crate) last_error: Option<String>,
}

impl ConfigReload {
    pub(crate) fn new(config: &Config, config_path: PathBuf, dirs: ReloadDirs) -> Self {
        Self {
            last_mtime: super::mtime(&config_path),
            config_path,
            last_check: Instant::now(),
            dirs,
            applied_language: super::learning_language(config),
            applied_dictionaries: config.dictionaries.clone(),
            applied_model: config.model.enabled,
            applied_input_log: config.general.input_log,
            pending: None,
            generation: 0,
            last_error: None,
        }
    }
}
