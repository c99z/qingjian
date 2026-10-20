//! 后台任务结果，基础分与语境分保持独立。

/// 后台算好的结果，与任务一一对应。
pub(crate) struct Scored {
    pub epoch: u64,

    pub semantic: Vec<String>,

    pub gains: Vec<f64>,

    pub context: String,

    pub keys: String,

    pub texts: Vec<String>,

    pub scores: Vec<f64>,

    /// 生成用的按键与生成出来的整句，对应任务里的 `generate`。
    pub generated: Option<(String, Vec<String>)>,
}
