//! 一个输入串下的累计选择与最近十六次选择位置；位零是最近一次。

#[derive(Debug, Default)]
pub(super) struct Choice {
    pub count: u32,

    pub recent: u16,
}

impl Choice {
    pub fn recent_score(mask: u16) -> f64 {
        let mut score = 0.0;
        let mut weight = 1.0;
        for bit in 0..u16::BITS {
            if mask & (1 << bit) != 0 {
                score += weight;
            }
            weight *= 0.75;
        }
        score
    }

    /// 从有界历史中删掉一次选择，后面的旧记录向前补齐。
    pub fn remove_recent(&mut self, position: u32) {
        let newer = self.recent & ((1u16 << position) - 1);
        let older = u32::from(self.recent) >> (position + 1);
        self.recent = newer | ((older << position) as u16);
    }
}
