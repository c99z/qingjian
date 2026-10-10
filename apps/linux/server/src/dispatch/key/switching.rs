//! 青简内部模式切换：单击修饰键在释放时触发，组合键在按下时触发。
use qingjian_platform::SwitchKey;
use qingjian_platform::protocol::{KeyEvent, SessionId};

use crate::dispatch::Router;

impl Router {
    pub(crate) fn mode_switch_key(
        &mut self,
        session: SessionId,
        event: &KeyEvent,
        release: bool,
    ) -> bool {
        let modifiers = event.modifiers;
        let key = match event.virtual_key {
            0x10 => Some(SwitchKey::Shift),
            0x11 => Some(SwitchKey::Control),
            _ => None,
        };
        let info = self.sessions.get_mut(&session).expect("session exists");
        if release {
            let pending = info.switch_pending.take();
            return key.is_some()
                && pending == key
                && key.is_some_and(|key| self.config.switch_mode.contains(key));
        }
        info.switch_pending = key.filter(|key| {
            self.config.switch_mode.contains(*key)
                && !modifiers.alt
                && !modifiers.win
                && match key {
                    SwitchKey::Shift => !modifiers.ctrl,
                    SwitchKey::Control => !modifiers.shift,
                    SwitchKey::CtrlAltSpace => false,
                }
        });
        self.config.switch_mode.ctrl_alt_space
            && event.virtual_key == 0x20
            && modifiers.ctrl
            && modifiers.alt
            && !modifiers.shift
            && !modifiers.win
    }
}
