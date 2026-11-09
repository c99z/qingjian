//! Linux 夜间个人词库整理调度；与云联想开关独立，默认不运行。

mod state;
mod worker;

use std::path::Path;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

use qingjian_platform::{Config, credentials, dirs::linux, extra_dictionaries};

use super::Router;
pub(super) use state::OrganizerState;

fn due(hour: u8, minute: u8, now: &jiff::Zoned, last: Option<&str>) -> bool {
    let day = now.date().to_string();
    let clock = i32::from(now.hour()) * 60 + i32::from(now.minute());
    let scheduled = i32::from(hour) * 60 + i32::from(minute);
    hour < 24
        && minute < 60
        && last != Some(day.as_str())
        && (0..240).contains(&(clock - scheduled))
}

fn read_day(dir: &Path) -> Option<String> {
    let bytes = std::fs::read(dir.join("organizer/last-attempt.json")).ok()?;
    serde_json::from_slice::<serde_json::Value>(&bytes).ok()?["day"]
        .as_str()
        .map(str::to_owned)
}

impl Router {
    pub(super) fn configure_organizer(&mut self, config: &Config) {
        let Some(reload) = &self.reload else { return };
        let api = match credentials::resolve(&config.predict, &reload.config_path) {
            Ok(api) => api,
            Err(_) => {
                self.organizer.cancel.store(true, Ordering::Release);
                self.organizer.settings = None;
                return;
            }
        };
        let enabled = config.general.learning && config.general.input_log;
        let settings = (config.organizer.clone(), api, enabled);
        if self.organizer.settings.as_ref() == Some(&settings) {
            return;
        }
        self.organizer.cancel.store(true, Ordering::Release);
        self.organizer.pending = None;
        self.organizer.settings = Some(settings);
        if !self.organizer.initialized {
            self.organizer.last_day = reload.dirs.user_dir.as_deref().and_then(read_day);
            self.organizer.initialized = true;
        }
    }

    pub(super) fn poll_organizer(&mut self) {
        if let Some(rx) = &self.organizer.running {
            match rx.try_recv() {
                Ok(result) => {
                    self.organizer.running = None;
                    if !self.organizer.cancel.load(Ordering::Acquire) {
                        match result {
                            Ok(draft) => self.organizer.pending = Some(draft),
                            Err(message) => self.organizer_status(&message, &[]),
                        }
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.organizer.running = None;
                    self.organizer_status("整理线程未完成，原词库保留", &[]);
                }
            }
        }
        if self.any_composing() {
            return;
        }
        if let Some((draft, domains)) = self.organizer.pending.take() {
            if let Err(message) = self.install_organized(&draft.tsv) {
                self.organizer_status(&message, &[]);
            } else {
                self.organizer_status(
                    &format!("已更新个人补充词库，{} 条；不修改基础词库", draft.words),
                    &domains,
                );
            }
        }
        if self.organizer.running.is_some()
            || self
                .organizer
                .last_check
                .is_some_and(|t| t.elapsed() < Duration::from_secs(30))
        {
            return;
        }
        self.organizer.last_check = Some(Instant::now());
        let Some((config, api, allowed)) = self.organizer.settings.clone() else {
            return;
        };
        let now = jiff::Zoned::now();
        if !config.enabled
            || !allowed
            || !due(
                config.hour,
                config.minute,
                &now,
                self.organizer.last_day.as_deref(),
            )
        {
            return;
        }
        // 私密窗口在前台时不启动整理；此次数据本来也不包含私密输入。
        if self
            .focused
            .and_then(|id| self.sessions.get(&id))
            .is_some_and(|s| s.active && (s.private || s.disabled))
        {
            return;
        }
        let Some(reload) = &self.reload else {
            return;
        };
        let (Some(data), Some(log)) = (reload.dirs.user_dir.clone(), reload.dirs.input_log.clone())
        else {
            return;
        };
        let Some(base) = linux::generated(&reload.dirs.resource_root, "dict.qj")
            .or_else(|| linux::asset(&reload.dirs.resource_root, "lexicon/dict.tsv"))
        else {
            return;
        };
        let day = now.date().to_string();
        let marker = serde_json::json!({"day":day});
        if credentials::write_private(
            &data.join("organizer/last-attempt.json"),
            marker.to_string().as_bytes(),
        )
        .is_err()
        {
            return;
        }
        self.organizer.last_day = Some(day);
        self.flush_learning();
        let cancel = Arc::new(AtomicBool::new(false));
        self.organizer.cancel = cancel.clone();
        let (tx, rx) = mpsc::sync_channel(1);
        match std::thread::Builder::new()
            .name("personal-organizer".into())
            .spawn(move || {
                let result = worker::run(&log, &base, &api, &config.domain_hint, &cancel);
                let _ = tx.send(result);
            }) {
            Ok(_) => {
                self.organizer.running = Some(rx);
                self.organizer_status("正在整理个人用词", &[]);
            }
            Err(_) => self.organizer_status("无法启动整理线程，原词库保留", &[]),
        }
    }

    fn organizer_status(&self, message: &str, domains: &[String]) {
        let Some(data) = self.reload.as_ref().and_then(|r| r.dirs.user_dir.as_ref()) else {
            return;
        };
        let value = serde_json::json!({"time":jiff::Zoned::now().to_string(),"message":message,"domains":domains});
        if credentials::write_private(
            &data.join("organizer/status.json"),
            value.to_string().as_bytes(),
        )
        .is_err()
        {
            tracing::warn!("无法保存个人词库整理状态");
        }
    }

    fn install_organized(&mut self, tsv: &str) -> Result<(), String> {
        let reload = self.reload.as_ref().ok_or("整理配置不存在")?;
        let data = reload.dirs.user_dir.as_ref().ok_or("用户数据目录不存在")?;
        let path = data.join("dicts/personal-agent.tsv");
        match std::fs::read(&path) {
            Ok(old) => credentials::write_private(&data.join("organizer/previous.tsv"), &old)
                .map_err(|_| "无法备份原个人补充词库")?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err("无法读取原个人补充词库".into()),
        }
        credentials::write_private(&path, tsv.as_bytes()).map_err(|_| "个人补充词库保存失败")?;
        let dictionaries = extra_dictionaries::load(
            reload.dirs.bundled_dicts.as_deref(),
            reload.dirs.user_dicts().as_deref(),
            &reload.applied_dictionaries,
        );
        self.engine.set_extra_dictionaries(dictionaries);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
