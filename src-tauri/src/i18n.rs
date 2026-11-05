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

const ES: Labels = Labels {
    off: "Desconectado",
    connecting: "Conectando…",
    on: "Conectado",
    stopping: "Desconectando…",
    error: "Sin conexión",
    connect: "Conectar",
    disconnect: "Desconectar",
    cancel: "Cancelar",
    open: "Abrir Donut Local VPN",
    autostart: "Iniciar con el sistema",
    quit: "Salir",
    lost: "La conexión dejó de funcionar. Buscando otra configuración…",
    restored: "Conectado de nuevo.",
    failed: "No se pudo reconectar. Abre Donut Local VPN para intentarlo de nuevo.",
    connected: "Conectado. Todo el tráfico pasa ahora por el motor local.",
    disconnected: "Desconectado.",
    not_connected: "No se pudo conectar. Abre Donut Local VPN para ver los detalles.",
};

const IT: Labels = Labels {
    off: "Disconnesso",
    connecting: "Connessione…",
    on: "Connesso",
    stopping: "Disconnessione…",
    error: "Non connesso",
    connect: "Connetti",
    disconnect: "Disconnetti",
    cancel: "Annulla",
    open: "Apri Donut Local VPN",
    autostart: "Avvia con il sistema",
    quit: "Esci",
    lost: "La connessione ha smesso di funzionare. Ricerca di un'altra impostazione…",
    restored: "Di nuovo connesso.",
    failed: "Impossibile riconnettersi. Apri Donut Local VPN per riprovare.",
    connected: "Connesso. Tutto il traffico passa ora dal motore locale.",
    disconnected: "Disconnesso.",
    not_connected: "Impossibile connettersi. Apri Donut Local VPN per i dettagli.",
};

const PL: Labels = Labels {
    off: "Wyłączono",
    connecting: "Łączenie…",
    on: "Połączono",
    stopping: "Rozłączanie…",
    error: "Brak połączenia",
    connect: "Połącz",
    disconnect: "Rozłącz",
    cancel: "Anuluj",
    open: "Otwórz Donut Local VPN",
    autostart: "Uruchamiaj z systemem",
    quit: "Zakończ",
    lost: "Połączenie przestało działać. Szukanie innego ustawienia…",
    restored: "Połączono ponownie.",
    failed: "Nie udało się połączyć ponownie. Otwórz Donut Local VPN, aby spróbować jeszcze raz.",
    connected: "Połączono. Cały ruch przechodzi teraz przez lokalny silnik.",
    disconnected: "Rozłączono.",
    not_connected: "Nie udało się połączyć. Otwórz Donut Local VPN, aby zobaczyć szczegóły.",
};

const KO: Labels = Labels {
    off: "꺼짐",
    connecting: "연결 중…",
    on: "연결됨",
    stopping: "연결 해제 중…",
    error: "연결되지 않음",
    connect: "연결",
    disconnect: "연결 해제",
    cancel: "취소",
    open: "Donut Local VPN 열기",
    autostart: "시스템과 함께 시작",
    quit: "종료",
    lost: "연결이 작동하지 않습니다. 다른 설정을 찾는 중…",
    restored: "다시 연결되었습니다.",
    failed: "다시 연결할 수 없습니다. Donut Local VPN을 열어 다시 시도하세요.",
    connected: "연결되었습니다. 이제 모든 트래픽이 로컬 엔진을 통해 전달됩니다.",
    disconnected: "연결이 해제되었습니다.",
    not_connected: "연결할 수 없습니다. 자세한 내용은 Donut Local VPN을 여세요.",
};

const ZH_TW: Labels = Labels {
    off: "已關閉",
    connecting: "正在連線…",
    on: "已連線",
    stopping: "正在中斷…",
    error: "未連線",
    connect: "連線",
    disconnect: "中斷連線",
    cancel: "取消",
    open: "開啟 Donut Local VPN",
    autostart: "開機時啟動",
    quit: "結束",
    lost: "連線中斷了，正在尋找其他設定…",
    restored: "已重新連線。",
    failed: "無法重新連線，請開啟 Donut Local VPN 再試一次。",
    connected: "已連線，所有流量現在都會經過本機引擎。",
    disconnected: "已中斷連線。",
    not_connected: "無法連線，請開啟 Donut Local VPN 查看詳情。",
};

const FR: Labels = Labels {
    off: "Déconnecté",
    connecting: "Connexion…",
    on: "Connecté",
    stopping: "Déconnexion…",
    error: "Non connecté",
    connect: "Se connecter",
    disconnect: "Se déconnecter",
    cancel: "Annuler",
    open: "Ouvrir Donut Local VPN",
    autostart: "Lancer au démarrage",
    quit: "Quitter",
    lost: "La connexion ne fonctionne plus. Recherche d'un autre réglage…",
    restored: "De nouveau connecté.",
    failed: "Impossible de se reconnecter. Ouvrez Donut Local VPN pour réessayer.",
    connected: "Connecté. Tout le trafic passe désormais par le moteur local.",
    disconnected: "Déconnecté.",
    not_connected: "Impossible de se connecter. Ouvrez Donut Local VPN pour plus de détails.",
};

pub fn labels(language: &str) -> &'static Labels {
    match language {
        "tr" => &TR,
        "ru" => &RU,
        "es" => &ES,
        "it" => &IT,
        "pl" => &PL,
        "ko" => &KO,
        "zh-TW" => &ZH_TW,
        "fr" => &FR,
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
