use crate::controller::Phase;

pub struct Labels {
    pub off: &'static str,
    pub connecting: &'static str,
    pub on: &'static str,
    pub stopping: &'static str,
    pub error: &'static str,
    pub connect: &'static str,
    pub disconnect: &'static str,
    pub cancel: &'static str,
    pub open: &'static str,
    pub autostart: &'static str,
    pub quit: &'static str,
    pub lost: &'static str,
    pub restored: &'static str,
    pub failed: &'static str,
    pub connected: &'static str,
    pub disconnected: &'static str,
    pub not_connected: &'static str,
}

const EN: Labels = Labels {
    off: "Off",
    connecting: "Connecting…",
    on: "Connected",
    stopping: "Disconnecting…",
    error: "Not connected",
    connect: "Connect",
    disconnect: "Disconnect",
    cancel: "Cancel",
    open: "Open Donut Local VPN",
    autostart: "Start with system",
    quit: "Quit",
    lost: "The connection stopped working. Looking for another setting…",
    restored: "Connected again.",
    failed: "Could not reconnect. Open Donut Local VPN to try again.",
    connected: "Connected. All traffic is now routed through the local engine.",
    disconnected: "Disconnected.",
    not_connected: "Could not connect. Open Donut Local VPN for details.",
};

const TR: Labels = Labels {
    off: "Kapalı",
    connecting: "Bağlanıyor…",
    on: "Bağlı",
    stopping: "Kapatılıyor…",
    error: "Bağlanamadı",
    connect: "Bağlan",
    disconnect: "Bağlantıyı kes",
    cancel: "İptal",
    open: "Donut Local VPN uygulamasını aç",
    autostart: "Sistemle birlikte başlat",
    quit: "Çıkış",
    lost: "Bağlantı çalışmayı bıraktı. Başka bir ayar aranıyor…",
    restored: "Yeniden bağlandı.",
    failed: "Yeniden bağlanılamadı. Tekrar denemek için Donut Local VPN uygulamasını aç.",
    connected: "Bağlandı. Tüm trafik artık yerel motordan geçiyor.",
    disconnected: "Bağlantı kesildi.",
    not_connected: "Bağlanılamadı. Ayrıntılar için Donut Local VPN uygulamasını aç.",
};

const RU: Labels = Labels {
    off: "Выключено",
    connecting: "Подключение…",
    on: "Подключено",
    stopping: "Отключение…",
    error: "Не подключено",
    connect: "Подключить",
    disconnect: "Отключить",
    cancel: "Отмена",
    open: "Открыть Donut Local VPN",
    autostart: "Запускать вместе с системой",
    quit: "Выход",
    lost: "Соединение перестало работать. Ищем другую настройку…",
    restored: "Снова подключено.",
    failed: "Не удалось переподключиться. Откройте Donut Local VPN, чтобы попробовать снова.",
    connected: "Подключено. Весь трафик теперь идёт через локальный движок.",
    disconnected: "Отключено.",
    not_connected: "Не удалось подключиться. Подробности в Donut Local VPN.",
};

pub fn labels(language: &str) -> &'static Labels {
    match language {
        "tr" => &TR,
        "ru" => &RU,
        _ => &EN,
    }
}

impl Labels {
    pub fn phase(&self, phase: Phase) -> &'static str {
        match phase {
            Phase::Off => self.off,
            Phase::Connecting => self.connecting,
            Phase::On => self.on,
            Phase::Stopping => self.stopping,
            Phase::Error => self.error,
        }
    }

    pub fn action(&self, phase: Phase) -> &'static str {
        match phase {
            Phase::Off | Phase::Error => self.connect,
            Phase::Connecting => self.cancel,
            Phase::On | Phase::Stopping => self.disconnect,
        }
    }
}
