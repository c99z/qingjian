//! 配置热重载：一秒节拍轮询 `config.toml` 的 mtime（原子替换按路径重读即可），管理接口的 Reload 立即触发。
//! 学习开关立即生效；输入方案、快捷键、候选数量、释义语言等影响组句的修改等所有会话的组句结束后应用——
//! 组句中保存设置，正在输入的拼音与候选不丢。解析失败保持上一次可用配置，错误进 `Status`。
//! 后台加载（模型）带配置代次，旧代次的结果不覆盖新设置。思路与 Windows Server 的 `dispatch/reload` 对齐。

mod state;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use qingjian_core::{Engine, Language, NoInputLogger, NoTranslator};
use qingjian_learning::InputLog;
use qingjian_platform::dirs::linux;
use qingjian_platform::settings::ComponentStatus;
use qingjian_platform::{Config, extra_dictionaries};

pub(crate) use self::state::ConfigReload;
pub use self::state::ReloadDirs;
use super::{Router, RouterConfig};
use crate::protocol::admin::ReloadReport;

/// 看配置文件 mtime 的最短间隔；空闲节拍本就是一秒，这里挡住重排短节拍时的频繁 stat。
const CONFIG_POLL_INTERVAL: Duration = Duration::from_secs(1);

fn mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
}

/// 配置指向的学习语言；`off` 为 `None`。与启动装配同一套解析。
pub(super) fn learning_language(config: &Config) -> Option<Language> {
    (!config.general.learning_language_off()).then(|| {
        config
            .general
            .learning_language
            .parse()
            .unwrap_or(Language::English)
    })
}

/// 学习语言的释义表路径（`.qj` 优先，回退随包 TSV）。
pub(super) fn glossary_path(root: &Path, language: Language) -> Option<PathBuf> {
    linux::generated(root, &format!("glossary-{}.qj", language.code()))
        .or_else(|| linux::asset(root, &format!("glossary/glossary-{}.tsv", language.code())))
}

impl Router {
    /// 开启热重载：记下配置路径与已应用的配置快照（语言 / 词库 / 模型 / 输入日志），以及数据目录。
    pub fn watch_config(&mut self, config: &Config, config_path: PathBuf, dirs: ReloadDirs) {
        self.reload = Some(ConfigReload::new(config, config_path, dirs));
        self.configure_cloud(&config.predict);
    }

    /// 当前配置代次；没开热重载（测试）恒为 0。
    pub(super) fn reload_generation(&self) -> u64 {
        self.reload.as_ref().map_or(0, |reload| reload.generation)
    }

    /// 空闲节拍调；一秒内只真正看一次。配置变了就重读应用；解析失败保持原配置，
    /// mtime 照记（不每秒重试同一个坏文件），错误留在 `Status` 里。
    pub(super) fn poll_config_reload(&mut self) {
        let Some(reload) = &mut self.reload else {
            return;
        };
        if reload.last_check.elapsed() < CONFIG_POLL_INTERVAL {
            return;
        }
        reload.last_check = Instant::now();
        let current = mtime(&reload.config_path);
        if current == reload.last_mtime {
            return;
        }
        reload.last_mtime = current;
        let path = reload.config_path.clone();
        self.reload_path(&path);
    }

    /// 管理接口的 `Reload`：立即读盘应用，不等轮询。
    pub(super) fn reload_now(&mut self) -> ReloadReport {
        let Some(path) = self
            .reload
            .as_ref()
            .map(|reload| reload.config_path.clone())
        else {
            return ReloadReport {
                generation: 0,
                server: ComponentStatus::Failed("服务未开启配置热重载".to_owned()),
            };
        };
        // 主动重载也刷新 mtime 与节流点，免得下一拍轮询把同一份配置再应用一遍。
        if let Some(reload) = &mut self.reload {
            reload.last_check = Instant::now();
            reload.last_mtime = mtime(&path);
        }
        self.reload_path(&path)
    }

    fn reload_path(&mut self, path: &Path) -> ReloadReport {
        match Config::load(path) {
            Ok(config) => {
                let server = self.stage_config(config);
                ReloadReport {
                    generation: self.reload_generation(),
                    server,
                }
            }
            Err(error) => {
                tracing::error!(%error, "配置重载解析失败，保持原配置");
                if let Some(reload) = &mut self.reload {
                    reload.last_error = Some(error.to_string());
                }
                ReloadReport {
                    generation: self.reload_generation(),
                    server: ComponentStatus::Failed(format!("配置解析失败：{error}")),
                }
            }
        }
    }

    /// 解析成功：学习开关立即生效；其余看组句边界，正在组句就先挂起（后来的配置整个替换先挂起的）。
    fn stage_config(&mut self, config: Config) -> ComponentStatus {
        self.engine.set_learning(config.general.learning);
        // 关闭云服务立即撤掉请求和候选，不等其他会话结束组句。
        if !config.predict.enabled {
            self.configure_cloud(&config.predict);
        }
        if self.any_composing() {
            if let Some(reload) = &mut self.reload {
                reload.pending = Some(Box::new(config));
                reload.last_error = None;
            }
            tracing::info!("组句进行中，配置等组句结束后应用");
            ComponentStatus::PendingReload
        } else {
            self.apply_config(&config);
            ComponentStatus::Applied
        }
    }

    /// 没有任何会话在组句时应用挂起的配置。消息处理完与每拍 tick 都看一次，不等轮询。
    pub(crate) fn maybe_apply_pending(&mut self) {
        if self.any_composing() {
            return;
        }
        let pending = self
            .reload
            .as_mut()
            .and_then(|reload| reload.pending.take());
        if let Some(config) = pending {
            self.apply_config(&config);
            tracing::info!(
                generation = self.reload_generation(),
                "挂起的配置已在组句结束后应用"
            );
        }
    }

    /// 聚焦或挂起会话有没有未上屏的组句；挂起会话也要检查，不能只看当前聚焦应用。
    pub(super) fn any_composing(&self) -> bool {
        self.composed.is_some()
            || self
                .sessions
                .values()
                .any(|session| session.composed.is_some())
    }

    /// 应用新配置（只在组句边界调）。便宜设置无条件重设；释义表 / 附加词库 / 模型 / 输入日志按 diff 换，
    /// 换失败保持旧资源。成功应用递增代次。
    fn apply_config(&mut self, config: &Config) {
        self.engine.set_fuzzy(config.fuzzy);
        self.engine.set_shuangpin(config.general.shuangpin());
        self.engine
            .set_shuangpin_raw_preedit(config.general.shuangpin_raw_preedit);
        self.engine.set_zhuyin_mode(config.general.is_zhuyin());
        self.engine
            .set_shift_letter_compose(config.general.shift_letter.compose());
        self.engine.set_learning(config.general.learning);
        self.engine.set_chinese_first(config.general.chinese_first);
        self.engine
            .set_chinese_input_style(config.general.chinese_input_style);
        self.engine.set_mode_keys(config.shortcut.mode);
        if let Err(error) = self
            .engine
            .set_custom_phrases(config.custom_phrases.clone())
        {
            tracing::warn!(%error, "自定义短语无效，保持原有短语");
        }
        // 展示设置刷新到 RouterConfig，新起的帧立即生效。
        let mut router_config = RouterConfig::from(config);
        router_config.page_size = router_config.page_size.clamp(1, 9);
        router_config.cloud_slots = 0;
        self.config = router_config;
        for session in self.sessions.values_mut() {
            session.switch_pending = None;
        }
        self.configure_cloud(&config.predict);
        if self.engine.prediction_enabled() {
            self.config.cloud_slots = config.predict.slots;
        }

        let language = learning_language(config);
        if self
            .reload
            .as_ref()
            .is_some_and(|reload| reload.applied_language != language)
        {
            swap_translator(&mut self.engine, language, &mut self.reload);
        }

        let dictionaries_changed = self.reload.is_some();
        if dictionaries_changed && let Some(reload) = &mut self.reload {
            let dictionaries = extra_dictionaries::load(
                reload.dirs.bundled_dicts.as_deref(),
                reload.dirs.user_dicts().as_deref(),
                &config.dictionaries,
            );
            tracing::info!(count = dictionaries.len(), "附加词库已热重装");
            self.engine.set_extra_dictionaries(dictionaries);
            reload.applied_dictionaries = config.dictionaries.clone();
        }

        let model_toggle = self
            .reload
            .as_ref()
            .map(|reload| (reload.applied_model, config.model.enabled));
        if let Some((applied, enabled)) = model_toggle
            && applied != enabled
        {
            if let Some(reload) = &mut self.reload {
                reload.applied_model = enabled;
            }
            if enabled {
                self.model_error = None;
                self.load_local_model();
            } else {
                // 关掉立即生效：不等加载中的模型，已装上的打分器也卸下。
                self.model_loader = None;
                self.engine.set_async_sentence_scorer(None);
                self.model_error = None;
                tracing::info!("本地整句模型已随配置关闭");
            }
        }

        let log_toggle = self
            .reload
            .as_ref()
            .map(|reload| (reload.applied_input_log, config.general.input_log));
        if let Some((applied, enabled)) = log_toggle
            && applied != enabled
        {
            if let Some(reload) = &mut self.reload {
                reload.applied_input_log = enabled;
            }
            let path = self
                .reload
                .as_ref()
                .and_then(|reload| reload.dirs.input_log.clone());
            match (enabled, path) {
                (true, Some(path)) => {
                    tracing::info!(path = %path.display(), "输入日志已随配置开启");
                    self.engine.set_input_logger(Box::new(InputLog::open(path)));
                }
                (true, None) => tracing::warn!("没有用户数据目录，输入日志保持关闭"),
                (false, _) => {
                    tracing::info!("输入日志已随配置关闭");
                    self.engine.set_input_logger(Box::new(NoInputLogger));
                }
            }
        }

        if let Some(reload) = &mut self.reload {
            reload.generation += 1;
            reload.last_error = None;
        }
    }
}

/// 学习语言变了就换释义表：关是不翻译；换语言重装随包 + 个人释义表，没有这门语言的表或装不上就保持原样。
fn swap_translator(
    engine: &mut Engine,
    language: Option<Language>,
    reload: &mut Option<ConfigReload>,
) {
    let Some(language) = language else {
        engine.set_translator(Box::new(NoTranslator));
        if let Some(reload) = reload {
            reload.applied_language = None;
        }
        tracing::info!("学习语言已关，不显示译文");
        return;
    };
    let Some(reload) = reload.as_mut() else {
        return;
    };
    let Some(path) = glossary_path(&reload.dirs.resource_root, language) else {
        tracing::warn!(
            language = language.code(),
            "没有这门语言的释义表，学习语言不变"
        );
        return;
    };
    match crate::assembly::load_glossary(language, &path, reload.dirs.user_dir.as_deref()) {
        Ok(glossary) => {
            tracing::info!(language = language.code(), "释义表已切换");
            engine.set_translator(Box::new(glossary));
            reload.applied_language = Some(language);
        }
        Err(error) => tracing::warn!(%error, "释义表加载失败，学习语言不变"),
    }
}
