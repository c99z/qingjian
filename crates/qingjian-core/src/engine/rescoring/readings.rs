//! 不同读音之间保留词图的纠错证据，同一读音内仍由神经模型完整重排。

use std::collections::HashMap;

use crate::sentence::Conversion;

/// 跨读音的模型增益最多改变两点 log 分；敲错代价和个人偏好已包含在基础分里。
/// P2C 以输入拼音为条件，不能再把漏键的纠正当成拼音不匹配而无限扣分。
const READING_MARGIN: f64 = 2.0;

pub(super) fn offsets(
    paths: &[Conversion],
    rescored: impl Fn(&Conversion) -> f64,
) -> HashMap<Vec<String>, f64> {
    let mut groups: HashMap<Vec<String>, (f64, f64)> = HashMap::new();
    for path in paths {
        let scores = groups
            .entry(path.syllables.clone())
            .or_insert((f64::NEG_INFINITY, f64::NEG_INFINITY));
        scores.0 = scores.0.max(path.score);
        scores.1 = scores.1.max(rescored(path));
    }
    let reference = paths
        .iter()
        .max_by(|a, b| a.score.total_cmp(&b.score))
        .and_then(|path| groups.get(&path.syllables))
        .map_or(0.0, |(base, neural)| neural - base);
    groups
        .into_iter()
        .map(|(reading, (base, neural))| {
            let gain = neural - base;
            let bounded = gain.clamp(reference - READING_MARGIN, reference + READING_MARGIN);
            (reading, bounded - gain)
        })
        .collect()
}
