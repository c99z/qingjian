//! 后台任务及语境候选池快照。

/// 一次后台任务：两个条件与一批要打分的文本，外加可选的「直接按这段按键生成整句」。
/// 两件事合成一条任务是因为它们都在用户停顿后一起发出，而排队的任务只算最新一条。
pub(super) struct Job {
    pub epoch: u64,

    pub semantic: Vec<String>,

    pub context: String,

    pub keys: String,

    pub texts: Vec<String>,

    /// 要生成整句的那段按键（整段作用域）；`None` 就只打分。
    pub generate: Option<String>,
}
