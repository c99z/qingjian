//! 附加候选：日期时间等快捷项、中英混输的英文词与补全、emoji。

use qingjian_dictionary::WordList;

use super::{
    EMOJI_PER_WORD, EMOJI_SCAN, EMOJI_TOTAL, ENGLISH_COMPLETIONS, Engine, Learner,
    MIN_COMPLETION_LETTERS,
};
use crate::candidate::{Candidate, CandidateKind};
use crate::{parser, ranking, shortcut};

impl Engine {
    /// 精确匹配自定义输入码时，数字键应选择候选。
    pub(super) fn has_custom_phrase(&self) -> bool {
        !self.english_mode
            && self
                .custom_phrases
                .iter()
                .any(|p| p.enabled && p.code == self.composition.scope())
    }

    /// 所有普通候选完成排序后按输入码固定位置。
    pub(super) fn insert_custom_phrases(&self, items: &mut Vec<Candidate>) {
        if self.english_mode {
            return;
        }
        let mut phrases: Vec<_> = self
            .custom_phrases
            .iter()
            .filter(|p| p.enabled && p.code == self.composition.scope())
            .collect();
        phrases.sort_by_key(|p| p.position);
        for phrase in phrases {
            items.insert(
                (phrase.position - 1).min(items.len()),
                Candidate {
                    text: phrase.text.clone(),
                    kind: CandidateKind::Custom(phrase.position),
                    syllables: Vec::new(),
                    reading: None,
                    translation: None,
                    aux_code: None,
                },
            );
        }
    }

    /// 日期 / 时间 / 星期这类快捷候选插在本地首选之后：`rq` 首选仍是词库里的词，快捷写法紧随其后。
    pub(super) fn insert_shortcuts(&self, items: &mut Vec<Candidate>, scope: &str) {
        let expression_char =
            if self.zhuyin && crate::zhuyin::layout::map_key(self.modes().expression).is_some() {
                '\0'
            } else {
                self.modes().expression
            };
        let shortcuts = shortcut::candidates(scope, expression_char, &jiff::Zoned::now());
        if shortcuts.is_empty() {
            return;
        }
        let position = items.len().min(1);
        items.splice(position..position, shortcuts);
    }

    /// 中英混输：整段输入是英文词就把它加进候选。
    /// 缺省作为拼音「不像话」（切不动、或除末尾外还有声母缩写 / 残缺音节）时排第一，否则排第二；
    /// 开了中文优先（`chinese_first`）首个中文候选在前，同输入下更常选的中文候选也不被英文挤开。没有中文候选时英文在第一。
    /// 两字母的全大写缩写（`mp` → MP、`bm` → BM）是个例外：整段太短、几乎总是在打中文（门票 / 编码），
    /// 这种让中文先；超过两个字母的正文英文（cargo / rust）照旧——按词频一刀切会把它们一起挤掉。
    /// 例外只管**没选过**的英文词：用户选过的照旧排第一（选过 OK，下次敲 `ok` 还是 OK 在前）。
    pub(super) fn insert_english(&self, items: &mut Vec<Candidate>, unlikely_pinyin: bool) {
        if !self.chinese_input_style.mixed() {
            return;
        }
        let lists = self.english_lists();
        if lists.is_empty() {
            return;
        }
        let text = self.composition.scope();
        if text.contains('\'') {
            return;
        }
        let english_candidate = |word: &str| Candidate {
            text: word.to_owned(),
            kind: CandidateKind::English,
            syllables: Vec::new(),
            reading: None,
            translation: None,
            aux_code: None,
        };
        let word = lists.iter().find_map(|words| {
            words
                .get(text)
                .filter(|word| self.mixed_english_allowed(word))
        });
        // 中英文都按同一输入下的偏好比较，不能拿中文的局部次数与英文的全局次数比较。
        // 拼音再不像话也是他自己教的
        let preference = |word: &str| {
            ranking::choice_bonus(self.learner.choice_weight(text, word))
                + ranking::RECENT_CHOICE_BONUS * self.learner.recent_choice_weight(text, word)
        };
        let chosen = items
            .iter()
            .find(|c| c.kind == CandidateKind::Chinese)
            .map_or(0.0, |c| preference(&c.text));
        let english_preference = word.map_or(0.0, preference);
        let english_weight = word.map_or(0, |w| self.learner.weight(w));
        // 两字母全大写缩写（mp → MP、bm → BM）让中文先：整段太短，几乎总是在打中文。
        // 但**不压过学习记录**：`ok` / `pc` / `ll` 同样满足「两个字母的全大写缩写」，一刀切会把它们一起
        // 翻成中文；而且选过 OK 的用户下次敲 `ok` 本该还是它排第一。所以这条只对用户**没选过**的英文词生效
        //（`record` 对英文候选也记次数，所以 `english_weight` 就是「选过没有」）。
        let short_acronym = english_weight == 0
            && text.len() <= 2
            && word.is_some_and(|word| word.chars().all(|c| c.is_ascii_uppercase()));
        let english_first = !self.chinese_first
            && unlikely_pinyin
            && chosen <= english_preference
            && !short_acronym;
        let mut position = if items.is_empty() || english_first {
            0
        } else {
            items
                .iter()
                .take_while(|c| {
                    c.kind == CandidateKind::Chinese && preference(&c.text) > english_preference
                })
                .count()
                .max(1)
        };
        if let Some(word) = word {
            items.insert(position, english_candidate(word));
            position += 1;
        }
        // 英文补全：拼音不像话时（`compa` 切成 co'm'pa），整段多半是在打英文词的前面几个字母，补全紧跟在精确词之后；
        // 个人词表在前，两张表里都有的只出一次。
        if unlikely_pinyin && text.len() >= MIN_COMPLETION_LETTERS {
            let mut budget = ENGLISH_COMPLETIONS;
            for words in &lists {
                for word in words.complete(text, budget) {
                    if !self.mixed_english_allowed(word) {
                        continue;
                    }
                    if items
                        .iter()
                        .any(|c| c.kind == CandidateKind::English && c.text == word)
                    {
                        continue;
                    }
                    items.insert(position, english_candidate(word));
                    position += 1;
                    budget -= 1;
                }
                if budget == 0 {
                    break;
                }
            }
        }
    }

    /// 给英文候选用的词表，个人的在前、随包的在后；一张都没有就是空。
    /// 整段作用域本身就是个英文词（`database`、`agent`）：用户多半在打那个词。
    pub(in crate::engine) fn scope_is_english_word(&self) -> bool {
        let scope = self.composition.scope();
        self.chinese_input_style.mixed()
            && !scope.is_empty()
            && self.english_lists().iter().any(|words| {
                words
                    .get(scope)
                    .is_some_and(|word| self.mixed_english_allowed(word))
            })
    }

    /// 个人表里能完整读成拼音的小写串，只有明确选过英文候选才参与中文混输。
    /// 随包英文词与英文模式照常可用，不删用户数据。
    pub(super) fn mixed_english_allowed(&self, word: &str) -> bool {
        if self.english_mode || !word.bytes().all(|b| b.is_ascii_lowercase()) {
            return true;
        }
        let code = word.to_ascii_lowercase();
        self.english
            .as_ref()
            .is_some_and(|words| words.get(&code).is_some())
            || self.learner.choice_weight(&code, word) > 0
            || !parser::is_fully_segmentable(word)
    }

    pub(super) fn english_lists(&self) -> Vec<&WordList> {
        self.learner
            .user_english()
            .into_iter()
            .chain(self.english.as_ref())
            .collect()
    }

    /// emoji 候选：前几个中文候选里有配 emoji 的，emoji 紧跟在那个词后面，右侧标注它对应的词。
    /// 词后面紧挨着的英文词候选（中文优先时 `key` → 可以、key）不被 emoji 挤开，emoji 排在它之后。
    pub(super) fn insert_emoji(&self, items: &mut Vec<Candidate>) {
        let Some(table) = &self.emoji else { return };
        let mut inserted = 0;
        let mut index = 0;
        let mut scanned = 0;
        while index < items.len() && scanned < EMOJI_SCAN && inserted < EMOJI_TOTAL {
            let item = &items[index];
            index += 1;
            if !matches!(
                item.kind,
                CandidateKind::Chinese | CandidateKind::Sentence | CandidateKind::English
            ) {
                continue;
            }
            scanned += 1;
            // 英文词按小写查英文表（smile → 😀）
            let key = if item.kind == CandidateKind::English {
                item.text.to_lowercase()
            } else {
                item.text.clone()
            };
            let emojis = table.lookup(&key);
            if emojis.is_empty() {
                continue;
            }
            let word = item.text.clone();
            let syllables = item.syllables.clone();
            if item.kind != CandidateKind::English {
                while items
                    .get(index)
                    .is_some_and(|next| next.kind == CandidateKind::English)
                {
                    index += 1;
                }
            }
            for emoji in emojis.iter().take(EMOJI_PER_WORD) {
                if inserted >= EMOJI_TOTAL {
                    break;
                }
                items.insert(
                    index,
                    Candidate {
                        text: emoji.clone(),
                        kind: CandidateKind::Emoji,
                        syllables: syllables.clone(),
                        reading: Some(word.clone()),
                        translation: None,
                        aux_code: None,
                    },
                );
                index += 1;
                inserted += 1;
            }
        }
    }
}
