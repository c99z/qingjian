//! 云服务线程装配及会话隔离；按键路径只收发内存通道。
mod state;
use super::{Router, composed::Composed};
use qingjian_core::{CloudWord, NoGlossFiller, NoPredictor};
use qingjian_platform::protocol::linux_admin::CloudStatus;
use qingjian_predict::{CloudGlossFiller, CloudPredictor, PredictConfig};
pub(crate) use state::CloudState;

impl Router {
    pub(super) fn configure_cloud(&mut self, config: &PredictConfig) {
        if config.enabled && (!config.valid_endpoint() || config.model.trim().is_empty()) {
            self.cancel_cloud();
            self.engine.set_predictor(Box::new(NoPredictor));
            self.engine.set_gloss_filler(Box::new(NoGlossFiller));
            self.config.cloud_slots = 0;
            self.cloud.config = None;
            self.cloud.status = CloudStatus::Failed("云服务地址或模型不合法".into());
            return;
        }
        let resolved = self.reload.as_ref().map_or_else(
            || Ok(config.clone()),
            |reload| qingjian_platform::credentials::resolve(config, &reload.config_path),
        );
        if let Ok(ref resolved) = resolved
            && self.cloud.config.as_ref() == Some(resolved)
        {
            return;
        }
        self.cancel_cloud();
        self.engine.set_predictor(Box::new(NoPredictor));
        self.engine.set_gloss_filler(Box::new(NoGlossFiller));
        self.config.cloud_slots = 0;
        match resolved {
            Ok(config) => {
                self.cloud.config = Some(config.clone());
                if !config.enabled {
                    self.cloud.status = CloudStatus::Disabled;
                    return;
                }
                match CloudPredictor::new(&config) {
                    Ok(predictor) => {
                        self.engine.set_predictor(Box::new(predictor));
                        if let Ok(filler) = CloudGlossFiller::new(&config) {
                            self.engine.set_gloss_filler(Box::new(filler));
                        }
                        self.config.cloud_slots = config.slots;
                        self.cloud.status = CloudStatus::Ready;
                    }
                    Err(_) => {
                        self.cloud.status =
                            CloudStatus::Failed("无法启用云服务，请检查本机 API Key".into())
                    }
                }
            }
            Err(message) => {
                self.cloud.config = None;
                self.cloud.status = CloudStatus::Failed(message);
            }
        }
    }
    pub(super) fn cancel_cloud(&mut self) {
        self.engine.cancel_prediction();
        self.cloud.owner = None;
        self.sentence = None;
        if let Some(Composed::Candidates { layout, .. }) = &mut self.composed {
            layout.set_cloud(Vec::new());
        }
    }
    pub(super) fn request_cloud(&mut self) {
        let Some(session) = self.focused else {
            return;
        };
        let info = &self.sessions[&session];
        if !info.active || info.private || info.disabled || info.capabilities.is_none() {
            self.cancel_cloud();
            return;
        }
        if let Some(Composed::Candidates { layout, .. }) = &self.composed {
            self.cloud.owner = self
                .engine
                .request_prediction(None, layout.local())
                .map(|_| session);
        } else {
            self.cancel_cloud();
        }
    }
    pub(super) fn poll_cloud(&mut self) {
        let Some(owner) = self.cloud.owner else {
            return;
        };
        if self.focused != Some(owner)
            || self
                .sessions
                .get(&owner)
                .is_none_or(|s| !s.active || s.private || s.disabled)
        {
            self.cancel_cloud();
            return;
        }
        let Some(prediction) = self.engine.poll_prediction() else {
            return;
        };
        self.cloud.owner = None;
        if let Some(Composed::Candidates { layout, .. }) = &mut self.composed {
            // 用户已移动高亮时不改变同一格所代表的候选。
            if !self.navigated {
                layout.set_cloud(
                    prediction
                        .words
                        .into_iter()
                        .map(CloudWord::into_candidate)
                        .collect(),
                );
            }
            self.sentence = prediction.sentence;
        }
    }
}
