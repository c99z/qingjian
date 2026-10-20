//! 候选正文与第一条译词；选词仍回 Server 调 Engine::commit。
#pragma once
#include <fcitx/candidatelist.h>
#include <functional>
namespace qingjian {
class Word final : public fcitx::CandidateWord {
public:
    Word(std::string text, std::string comment, std::function<void(fcitx::InputContext *)> select,
         bool stacked)
        : CandidateWord(fcitx::Text(text)), select_(std::move(select)) {
        setPlaceHolder(text.empty());
        if (stacked && !text.empty()) {
            // 默认面板把正文与 comment 并排；合成多行正文才能让译词位于中文正下方。
            // 译词只供展示，不能进入前端的正文提交。
            fcitx::Text display(std::move(text));
            display.append("\n" + comment, fcitx::TextFormatFlag::DontCommit);
            setText(std::move(display));
        } else {
            setComment(fcitx::Text(std::move(comment)));
        }
    }
    void select(fcitx::InputContext *context) const override { if (!isPlaceHolder()) select_(context); }
private:
    /// 含帧版本检查的选择操作。
    std::function<void(fcitx::InputContext *)> select_;
};
}
