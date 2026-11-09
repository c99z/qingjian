//! 根据经过验证的证据生成低权重补充词库，不覆盖原词频与个人选择记录。

use qingjian_dictionary::Dictionary;

use super::{WordEvidence, valid_word};

pub struct OrganizationDraft {
    pub tsv: String,

    pub words: usize,
}

impl OrganizationDraft {
    pub fn build(
        evidence: &[WordEvidence],
        accepted: &[usize],
        base: &Dictionary,
    ) -> Result<Self, String> {
        let existing: std::collections::BTreeSet<_> =
            base.entries().map(|h| (h.text, h.pinyin)).collect();
        let mut seen = std::collections::BTreeSet::new();
        let mut tsv =
            String::from("# 青简夜间整理的个人补充词；由本机已确认输入提取，不修改基础词库\n");
        let mut words = 0;
        for id in accepted {
            let e = evidence.get(*id).ok_or("词库整理编号越界")?;
            if !seen.insert(*id) || !valid_word(&e.word, &e.pinyin) || e.count < 3 || e.days < 2 {
                return Err("词库整理证据不合法".into());
            }
            if existing.contains(&(e.word.as_str(), e.pinyin.as_str())) {
                continue;
            }
            // 累计选词和最近偏好仍由原学习器处理，新词只获得有限静态底值。
            tsv.push_str(&format!("{}\t{}\t{}\n", e.word, e.pinyin, e.count.min(30)));
            words += 1;
        }
        Dictionary::parse(&tsv).map_err(|_| "个人补充词库校验失败")?;
        Ok(Self { tsv, words })
    }
}
