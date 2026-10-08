//! 管理服务协调一致备份与恢复；固定目录，命令只接受不含路径的传输标识。
use super::Router;
use qingjian_learning::{FrequencyLearner, UsageStats};
use qingjian_platform::backup::{Archive, BackupPaths, RestoreGuard};
use qingjian_platform::settings::ComponentStatus;

impl Router {
    fn backup_paths(&self) -> Result<BackupPaths, String> {
        let reload = self.reload.as_ref().ok_or("服务没有配置热重载信息")?;
        let data = reload.dirs.user_dir.clone().ok_or("服务没有用户数据目录")?;
        let themes = data.parent().unwrap_or(&data).join("fcitx5/themes");
        Ok(BackupPaths {
            config: reload.config_path.clone(),
            data,
            themes,
        })
    }
    pub(super) fn create_backup(&mut self) -> Result<String, String> {
        let paths = self.backup_paths()?;
        self.engine.flush_learning_checked()?;
        let archive = Archive::capture(&paths)?;
        let path = paths
            .data
            .join("backups")
            .join(format!("{}.qjb", uuid::Uuid::new_v4()));
        archive.write(&path)?;
        Ok(path.display().to_string())
    }
    pub(super) fn restore_backup(&mut self, transfer: &str) -> ComponentStatus {
        match self.restore_data(transfer) {
            Ok(message) => {
                tracing::info!(%message, "用户数据恢复成功");
                ComponentStatus::Applied
            }
            Err(message) => ComponentStatus::Failed(message),
        }
    }
    fn restore_data(&mut self, transfer: &str) -> Result<String, String> {
        if uuid::Uuid::parse_str(transfer).is_err() || transfer.contains(['/', '\\']) {
            return Err("备份传输标识不合法".into());
        }
        if self.any_composing() {
            return Err("仍有未上屏的输入，请完成所有窗口的输入后重试恢复".into());
        }
        let paths = self.backup_paths()?;
        let input = paths.data.join("transfers").join(format!("{transfer}.qjb"));
        if input.parent().is_some_and(|p| p.is_symlink()) {
            return Err("传输目录不能是符号链接".into());
        }
        let archive = Archive::read(&input)?;
        let previous = self.create_backup()?;
        let mut guard = RestoreGuard::prepare(&archive, &paths)?;
        let prepared = (|| {
            let config = qingjian_platform::Config::load(&paths.config)
                .map_err(|_| "恢复配置加载失败".to_owned())?;
            let learner = FrequencyLearner::from_path(paths.data.join("user.tsv"))
                .map_err(|_| "恢复学习数据加载失败".to_owned())?;
            let levels = self
                .reload
                .as_ref()
                .map(|r| r.dirs.resource_root.join("assets/levels"));
            let vocabulary = crate::assembly::load_vocabulary(&paths.data, levels.as_deref());
            let language = crate::dispatch::reload::learning_language(&config);
            let translator: Box<dyn qingjian_core::Translator> = match language {
                None => Box::new(qingjian_core::NoTranslator),
                Some(language) => {
                    let root = &self
                        .reload
                        .as_ref()
                        .ok_or("服务重载信息缺失")?
                        .dirs
                        .resource_root;
                    let file = crate::dispatch::reload::glossary_path(root, language)
                        .ok_or("恢复所需的释义表未安装")?;
                    Box::new(
                        crate::assembly::load_glossary(language, &file, Some(&paths.data))
                            .map_err(|_| "恢复个人释义表失败")?,
                    )
                }
            };
            Ok::<_, String>((config, learner, vocabulary, translator, language))
        })();
        let (config, learner, vocabulary, translator, language) = match prepared {
            Ok(values) => values,
            Err(message) => {
                guard.rollback()?;
                return Err(format!("{message}，已回滚；恢复前备份：{previous}"));
            }
        };
        // 整个命令在主线程串行运行，没有持久维护锁；连接关闭后也不会留下一把锁。
        self.cancel_cloud();
        for info in self.sessions.values_mut() {
            info.engine.discard_input();
            info.composed = None;
        }
        self.engine.discard_input();
        self.engine.replace_user_data(
            Box::new(learner),
            Box::new(UsageStats::open(paths.data.join("usage.tsv"))),
            Box::new(vocabulary),
        );
        self.engine.set_translator(translator);
        if let Some(reload) = &mut self.reload {
            reload.applied_language = language;
        }
        self.reload_now();
        // 即使恢复语言未变，也必须重读个人释义表。
        if let Some(reload) = &mut self.reload {
            reload.pending = None;
        }
        self.configure_cloud(&config.predict);
        guard.commit();
        let _ = std::fs::remove_file(input);
        Ok(format!("恢复完成；恢复前备份：{previous}"))
    }
}
