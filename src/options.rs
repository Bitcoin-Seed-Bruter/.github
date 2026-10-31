use std::ffi::OsString;
use std::path::PathBuf;

pub const HELP: &str = "seed-demo — ДЕМОНСТРАЦИОННЫЙ симулятор, результаты вымышлены\n\n\
Использование: seed-demo [--wallets ПУТЬ] [--seconds N] [--plain] [--report ПУТЬ]\n\n\
  --wallets ПУТЬ  Текстовый список меток/адресов; по умолчанию 3 учебные метки\n\
  --seconds N     Длительность: от 1 до 3600 секунд (по умолчанию 15)\n\
  --plain         Обычные строки без анимации, удобно для перенаправления вывода\n\
  --report ПУТЬ   Создать текстовый демо-отчёт; существующий файл не заменяется\n\
  -h, --help      Справка\n\
  -V, --version   Версия\n\n\
Программа работает локально. Токены demo-xxxx не являются seed-фразами BIP39.\n";

pub struct Options {
    pub wallets: Option<PathBuf>,
    pub seconds: u64,
    pub plain: bool,
    pub report: Option<PathBuf>,
}

pub enum Action {
    Run(Options),
    Help,
    Version,
}

pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Action, String> {
    let mut args = args.into_iter();
    let mut options = Options {
        wallets: None,
        seconds: 15,
        plain: false,
        report: None,
    };
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("-h" | "--help") => return Ok(Action::Help),
            Some("-V" | "--version") => return Ok(Action::Version),
            Some("--plain") => options.plain = true,
            Some("--wallets" | "--report" | "--seconds") => {
                let value = args
                    .next()
                    .ok_or_else(|| format!("для {arg:?} нужно значение"))?;
                if value.is_empty() || value.to_string_lossy().starts_with("--") {
                    return Err(format!("для {arg:?} нужно значение"));
                }
                match arg.to_str() {
                    Some("--wallets") => options.wallets = Some(PathBuf::from(value)),
                    Some("--report") => options.report = Some(PathBuf::from(value)),
                    _ => {
                        options.seconds = value
                            .to_str()
                            .and_then(|text| text.parse::<u64>().ok())
                            .filter(|seconds| (1..=3600).contains(seconds))
                            .ok_or("--seconds: нужно целое число от 1 до 3600")?;
                    }
                }
            }
            _ => return Err(format!("неизвестный аргумент {arg:?}; справка: --help")),
        }
    }
    Ok(Action::Run(options))
}
