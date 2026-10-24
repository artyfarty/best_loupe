//! UI strings. Russian when the Windows UI language is Russian, English otherwise.

use std::sync::OnceLock;

pub struct Strings {
    pub tip: &'static str,
    pub menu_settings: &'static str,
    pub menu_about: &'static str,
    pub menu_update: &'static str,
    pub menu_exit: &'static str,
    pub window_title: &'static str,
    pub tab_lens: &'static str,
    pub tab_zoom: &'static str,
    pub tab_general: &'static str,
    pub tab_about: &'static str,
    pub lens_width: &'static str,
    pub lens_height: &'static str,
    pub corner_radius: &'static str,
    pub border_width: &'static str,
    pub border_color: &'static str,
    pub choose_color: &'static str,
    pub shadow: &'static str,
    pub initial_zoom: &'static str,
    pub max_zoom: &'static str,
    pub zoom_step: &'static str,
    pub zoom_smoothing: &'static str,
    pub smoothing: &'static str,
    pub hotkey: &'static str,
    pub hotkey_hint: &'static str,
    pub activation_delay: &'static str,
    pub fade_in: &'static str,
    pub autostart: &'static str,
    pub check_updates: &'static str,
    pub open_config: &'static str,
    pub reset_defaults: &'static str,
    pub tagline: &'static str,
    pub version: &'static str,
    pub howto: &'static str,
    pub check_now: &'static str,
    pub download: &'static str,
    pub project_page: &'static str,
    pub open_log: &'static str,
    pub upd_idle: &'static str,
    pub upd_disabled: &'static str,
    pub upd_checking: &'static str,
    pub upd_latest: &'static str,
    pub upd_available: &'static str,
    pub upd_failed: &'static str,
    pub upd_balloon_title: &'static str,
    pub upd_balloon_text: &'static str,
    pub off: &'static str,
}

static EN: Strings = Strings {
    tip: "best_loupe — hold {hotkey}, scroll to zoom",
    menu_settings: "Settings…",
    menu_about: "About best_loupe",
    menu_update: "Download update {version}…",
    menu_exit: "Exit",
    window_title: "best_loupe",
    tab_lens: "Lens",
    tab_zoom: "Zoom",
    tab_general: "General",
    tab_about: "About",
    lens_width: "Width",
    lens_height: "Height",
    corner_radius: "Rounding",
    border_width: "Border",
    border_color: "Border colour",
    choose_color: "Choose…",
    shadow: "Shadow",
    initial_zoom: "Zoom at start",
    max_zoom: "Maximum zoom",
    zoom_step: "Zoom per wheel notch",
    zoom_smoothing: "Zoom animation",
    smoothing: "Smooth the magnified image (off = crisp pixels)",
    hotkey: "Hold to magnify",
    hotkey_hint: "Scroll the wheel while holding to change the zoom.",
    activation_delay: "Show after",
    fade_in: "Fade-in",
    autostart: "Start with Windows",
    check_updates: "Check for updates automatically",
    open_config: "Open config folder",
    reset_defaults: "Reset to defaults",
    tagline: "The magnifier you have deserved for years — and were never given.",
    version: "Version {version} ({build})",
    howto: "Hold {hotkey} — a lens follows the cursor. Scroll to zoom. Let go — it is gone.",
    check_now: "Check now",
    download: "Download {version}",
    project_page: "Project page",
    open_log: "Open log",
    upd_idle: "Updates: not checked yet.",
    upd_disabled: "Automatic update checks are off.",
    upd_checking: "Checking for updates…",
    upd_latest: "You have the latest version.",
    upd_available: "Version {version} is available.",
    upd_failed: "Update check failed: {error}",
    upd_balloon_title: "best_loupe {version} is available",
    upd_balloon_text: "Click to open the download page.",
    off: "off",
};

static RU: Strings = Strings {
    tip: "best_loupe — держите {hotkey}, колесо меняет масштаб",
    menu_settings: "Настройки…",
    menu_about: "О программе",
    menu_update: "Скачать обновление {version}…",
    menu_exit: "Выход",
    window_title: "best_loupe",
    tab_lens: "Линза",
    tab_zoom: "Масштаб",
    tab_general: "Общие",
    tab_about: "О программе",
    lens_width: "Ширина",
    lens_height: "Высота",
    corner_radius: "Скругление",
    border_width: "Рамка",
    border_color: "Цвет рамки",
    choose_color: "Выбрать…",
    shadow: "Тень",
    initial_zoom: "Масштаб при запуске",
    max_zoom: "Максимальный масштаб",
    zoom_step: "Шаг колеса",
    zoom_smoothing: "Анимация масштаба",
    smoothing: "Сглаживать увеличенное изображение (выкл. — чёткие пиксели)",
    hotkey: "Удерживать для лупы",
    hotkey_hint: "Крутите колесо, удерживая клавиши, чтобы менять масштаб.",
    activation_delay: "Показывать через",
    fade_in: "Появление",
    autostart: "Запускать вместе с Windows",
    check_updates: "Автоматически проверять обновления",
    open_config: "Открыть папку настроек",
    reset_defaults: "Сбросить настройки",
    tagline: "Лупа, которую вы давно заслужили, но которую вам не давали.",
    version: "Версия {version} ({build})",
    howto: "Держите {hotkey} — за курсором следует линза. Колесо меняет масштаб. Отпустили — линза исчезла.",
    check_now: "Проверить",
    download: "Скачать {version}",
    project_page: "Страница проекта",
    open_log: "Открыть лог",
    upd_idle: "Обновления ещё не проверялись.",
    upd_disabled: "Автоматическая проверка обновлений выключена.",
    upd_checking: "Проверяю обновления…",
    upd_latest: "У вас последняя версия.",
    upd_available: "Доступна версия {version}.",
    upd_failed: "Не удалось проверить обновления: {error}",
    upd_balloon_title: "Доступна best_loupe {version}",
    upd_balloon_text: "Нажмите, чтобы открыть страницу загрузки.",
    off: "выкл.",
};

pub fn t() -> &'static Strings {
    static PICK: OnceLock<&'static Strings> = OnceLock::new();
    PICK.get_or_init(|| {
        let lang = unsafe { windows::Win32::Globalization::GetUserDefaultUILanguage() };
        if lang & 0x3ff == 0x19 { &RU } else { &EN }
    })
}

/// Human-readable hotkey of the current config, e.g. "Ctrl+Alt".
pub fn hotkey_label(cfg: &crate::config::Config) -> String {
    let mut parts = Vec::new();
    if cfg.hotkey_ctrl {
        parts.push("Ctrl");
    }
    if cfg.hotkey_alt {
        parts.push("Alt");
    }
    if cfg.hotkey_shift {
        parts.push("Shift");
    }
    parts.join("+")
}
