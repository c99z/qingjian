//! 真 Rust Server 与 Fcitx 默认面板的协议/中文输入闭环，避免 mock 隐藏版本漂移。
#include "qingjian.h"
#include "support/context.h"
#include <fcitx/inputcontextmanager.h>
#include <fcitx/inputpanel.h>
#include <cassert>
#include <chrono>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <thread>
#include <sys/wait.h>
#include <signal.h>
#include <unistd.h>
int main(int argc, char **argv) {
    assert(argc == 3);
    char directory[] = "/tmp/qingjian-real-XXXXXX";
    assert(mkdtemp(directory));
    std::string socketPath = std::string(directory) + "/server.sock";
    std::string dictionary = std::string(directory) + "/dict.tsv";
    {
        std::ifstream source(std::string(argv[2]) + "/assets/sample/dict.tsv");
        std::ofstream output(dictionary);
        assert(source && output);
        output << source.rdbuf() << "\n再\tzai\t100\n在\tzai\t90\n";
    }
    setenv("QINGJIAN_SOCKET", socketPath.c_str(), 1);
    auto sampleDir = std::filesystem::path(directory) / "assets/sample";
    std::filesystem::create_directories(sampleDir);
    std::filesystem::copy_file(std::filesystem::path(argv[2]) / "assets/sample/english.tsv", sampleDir / "english.tsv");
    auto glossaryDir = std::filesystem::path(directory) / "assets/glossary";
    std::filesystem::create_directories(glossaryDir);
    std::filesystem::copy_file(std::filesystem::path(argv[2]) / "assets/sample/glossary-en.tsv", glossaryDir / "glossary-en.tsv");
    setenv("QINGJIAN_RESOURCES", directory, 1);
    setenv("QINGJIAN_DICT", dictionary.c_str(), 1);
    setenv("XDG_CONFIG_HOME", directory, 1);
    setenv("XDG_DATA_HOME", directory, 1);
    setenv("XDG_STATE_HOME", directory, 1);
    unsetenv("DBUS_SESSION_BUS_ADDRESS");
    auto server = fork();
    assert(server >= 0);
    if (server == 0) { execl(argv[1], argv[1], nullptr); _exit(127); }
    for (int i = 0; i < 500 && !std::filesystem::exists(socketPath); ++i) std::this_thread::sleep_for(std::chrono::milliseconds(10));
    assert(std::filesystem::exists(socketPath));
    {
        char program[] = "qingjian-test"; char disable[] = "--disable=all";
        char *arguments[] = {program, disable, nullptr};
        fcitx::Instance instance(2, arguments);
        instance.initialize();
        fcitx::QingjianEngine engine(&instance.addonManager());
        Context context(instance.inputContextManager());
        context.focusIn(); // 全空 capability 应正常输入。
        // 用真实插件转发退格的按下 / 释放；退格必须透传给应用，重新选词后撤回旧学习。
        for (const auto &word : {"再", "在"}) {
            for (char c : std::string("zai")) assert(engine.process(&context, fcitx::Key(static_cast<fcitx::KeySym>(c))));
            auto choices = context.inputPanel().candidateList();
            assert(choices);
            int index = 0;
            while (index < choices->size() && choices->candidate(index).text().toString() != word) ++index;
            assert(index < choices->size());
            choices->candidate(index).select(&context);
            assert(context.committed == word);
            if (std::string(word) == "再") {
                assert(!engine.process(&context, fcitx::Key(FcitxKey_BackSpace)));
                assert(!engine.process(&context, fcitx::Key(FcitxKey_BackSpace), true));
            }
            context.committed.clear(); // 模拟应用处理透传退格。
        }
        for (char c : std::string("zai")) assert(engine.process(&context, fcitx::Key(static_cast<fcitx::KeySym>(c))));
        assert(context.inputPanel().candidateList()->candidate(0).text().toString() == "在");
        assert(engine.process(&context, fcitx::Key(FcitxKey_Escape)));
        for (char c : std::string("nihao")) assert(engine.process(&context, fcitx::Key(static_cast<fcitx::KeySym>(c))));
        auto list = context.inputPanel().candidateList();
        assert(list && list->size() > 0 && list->candidate(0).text().toString() == "你好");
        list->candidate(0).select(&context);
        assert(context.committed == "你好");
        list->candidate(0).select(&context);
        assert(context.committed == "你好");
        context.committed.clear();
        for (char c : std::string("nihao")) assert(engine.process(&context, fcitx::Key(static_cast<fcitx::KeySym>(c))));
        engine.process(&context, fcitx::Key(FcitxKey_Shift_L));
        assert(engine.process(&context, fcitx::Key(FcitxKey_space, fcitx::KeyState::Shift)));
        engine.process(&context, fcitx::Key(FcitxKey_Shift_L), true);
        assert(context.committed == "hello");
        // 组合键用过 Shift，松开不能切到英文；普通空格继续上屏中文。
        for (char c : std::string("nihao")) assert(engine.process(&context, fcitx::Key(static_cast<fcitx::KeySym>(c))));
        assert(engine.process(&context, fcitx::Key(FcitxKey_space)));
        assert(context.committed == "hello你好");
        // 同一上下文失焦 / 回焦保留 Server 的英文模式；框架 reset 只清输入。
        engine.process(&context, fcitx::Key(FcitxKey_Shift_L));
        assert(engine.process(&context, fcitx::Key(FcitxKey_Shift_L), true));
        context.focusOut();
        fcitx::InputMethodEntry entry("qingjian", "qingjian", "zh_CN", "qingjian");
        fcitx::FocusOutEvent out(&context);
        engine.deactivate(entry, out);
        context.focusIn();
        engine.process(&context, fcitx::Key(FcitxKey_h));
        assert(context.inputPanel().preedit().toString() == "h");
        assert(context.inputPanel().candidateList() && context.inputPanel().candidateList()->candidate(0).text().toString() == "hello");
        fcitx::InputContextEvent reset(&context, fcitx::EventType::InputContextReset);
        engine.reset(entry, reset);
        assert(context.inputPanel().empty());
        context.setCapabilityFlags(fcitx::CapabilityFlag::Password);
        assert(!engine.process(&context, fcitx::Key(FcitxKey_n)));
        assert(context.inputPanel().empty());
    }
    kill(server, SIGTERM);
    int status = 0;
    waitpid(server, &status, 0);
    assert(WIFEXITED(status) && WEXITSTATUS(status) == 0);
    // SIGTERM 正常保存后仍没有误选的偏好；后来的选择已落盘。
    std::ifstream choices(std::filesystem::path(directory) / "qingjian/user-choices.tsv");
    assert(choices);
    std::string saved((std::istreambuf_iterator<char>(choices)), std::istreambuf_iterator<char>());
    assert(saved.find("zai\t再\t") == std::string::npos);
    assert(saved.find("zai\t在\t1\t") != std::string::npos);
    std::filesystem::remove_all(directory);
}
