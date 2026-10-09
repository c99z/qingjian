//! Fcitx5 薄插件：框架事件转发、默认面板与上屏适配。
#pragma once
#include "session.h"
#include <fcitx-config/configuration.h>
#include <fcitx-config/option.h>
#include <fcitx/action.h>
#include <fcitx/addonfactory.h>
#include <fcitx/inputcontext.h>
#include <fcitx/inputmethodengine.h>
#include <fcitx/inputcontextproperty.h>
#include <fcitx/instance.h>
namespace fcitx {
/// 只挂一个启动设置应用的 ExternalOption：configtool 里点「配置」弹出的对话框就是它的启动按钮，
/// 真正的配置都在 GTK 设置应用里，addon 自身没有要存的项。
FCITX_CONFIGURATION(QingjianConfig,
                    ExternalOption settings{this, "Settings", "打开青简设置", "qingjian-linux-settings"};);
class QingjianEngine final : public InputMethodEngineV2 {
public:
    explicit QingjianEngine(AddonManager *manager);
    ~QingjianEngine() override;
    /// 把「青简设置」挂到状态区（InputMethod 组在 activate 前会被清空），托盘菜单才显示它。
    void activate(const InputMethodEntry &, InputContextEvent &event) override;
    const Configuration *getConfig() const override { return &config_; }
    void setConfig(const RawConfig &config) override { config_.load(config, true); }
    void keyEvent(const InputMethodEntry &, KeyEvent &) override;
    void reset(const InputMethodEntry &, InputContextEvent &) override;
    void deactivate(const InputMethodEntry &, InputContextEvent &) override;
    bool process(InputContext *context, const Key &key, bool release = false);
private:
    void clear(InputContext *context);
    void disconnect(InputContext *context);
    void disconnectAll();
    bool connect(InputContext *context);
    bool syncPrivacy(InputContext *context);
    bool exchange(InputContext *context, const nlohmann::json &event, bool display = true);
    void render(InputContext *context, const nlohmann::json &frame);
    /// 组句期间定时向 Server 发 Poll 取异步结果（本地整句模型重排）；帧为空就停。
    void watch(InputContext *context, bool composing);
    void stopPolling();
    void poll();

    Instance *instance_;

    QingjianConfig config_;

    /// 托盘菜单里的「青简设置」入口，触发时启动 GTK 设置应用。
    SimpleAction settingsAction_;

    std::shared_ptr<qingjian::SharedConnection> shared_;

    std::shared_ptr<bool> alive_ = std::make_shared<bool>(true);

    FactoryFor<qingjian::Session> sessions_;

    std::unique_ptr<HandlerTableEntry<EventHandler>> capabilityWatcher_;

    std::unique_ptr<HandlerTableEntry<EventHandler>> focusWatcher_;

    std::unique_ptr<HandlerTableEntry<EventHandler>> keyboardWatcher_;

    /// 轮询定时器：建一次反复用，没在组句时禁用。
    std::unique_ptr<EventSourceTime> poller_;

    /// 正在轮询的上下文：只有聚焦且在组句的那一个。
    TrackableObjectReference<InputContext> polled_;
};
}
