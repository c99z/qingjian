//! Linux 云服务装配状态与请求所属会话。
use qingjian_platform::protocol::SessionId;
use qingjian_platform::protocol::linux_admin::CloudStatus;
use qingjian_predict::PredictConfig;

pub(crate) struct CloudState {
    pub(super) config: Option<PredictConfig>,

    pub(super) owner: Option<SessionId>,

    pub(crate) status: CloudStatus,
}
impl Default for CloudState {
    fn default() -> Self {
        Self {
            config: None,
            owner: None,
            status: CloudStatus::Disabled,
        }
    }
}
