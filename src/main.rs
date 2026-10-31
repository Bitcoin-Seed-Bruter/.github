mod options;
mod wallets;

use options::{Action, Options};
use std::fs::OpenOptions;
use std::io::{self, IsTerminal, Write};
use std::process::ExitCode;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const DEMO_BANNER: &str = "ДЕМОНСТРАЦИОННЫЙ РЕЖИМ — все результаты вымышлены";
const DEMO_RATE: u64 = 125_000;
const FRAME_TIME: Duration = Duration::from_millis(100);

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Ошибка: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let options = match options::parse(std::env::args_os().skip(1))? {
        Action::Help => {
            print!("{}", options::HELP);
            return Ok(());
        }
        Action::Version => {
            println!(
                "seed-demo {} — демонстрационный симулятор",
                env!("CARGO_PKG_VERSION")
            );
            return Ok(());
        }
        Action::Run(options) => options,
    };
    let wallets = wallets::load(options.wallets.as_deref())?;
    // Create exclusively before starting, so an existing report is never overwritten.
    let mut report = options
        .report
        .as_ref()
        .map(|path| {
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .map_err(|error| format!("не удалось создать отчёт {}: {error}", path.display()))
        })
        .transpose()?;
    if let Some(file) = report.as_mut() {
        writeln!(
            file,
            "{DEMO_BANNER}\nНезавершённый запуск. Ожидайте итогового отчёта."
        )
        .map_err(|error| format!("не удалось записать отчёт: {error}"))?;
    }
    let stdout = io::stdout();
    let plain = options.plain || !stdout.is_terminal();
    let mut output = stdout.lock();
    let summary = simulate(&mut output, &wallets, &options, plain)
        .map_err(|error| format!("не удалось вывести результат: {error}"))?;
    if let Some(mut file) = report {
        use std::io::{Seek, SeekFrom};
        file.set_len(0)
            .and_then(|()| file.seek(SeekFrom::Start(0)).map(|_| ()))
            .and_then(|()| file.write_all(summary.as_bytes()))
            .and_then(|()| file.sync_all())
            .map_err(|error| format!("не удалось сохранить отчёт: {error}"))?;
        // The path may contain terminal control characters; print its escaped form.
        writeln!(
            output,
            "Демонстрационный отчёт: {:?}",
            options.report.unwrap()
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(())
}

struct DemoResult {
    label: String,
    tokens: String,
    satoshis: u64,
}

// This PRNG only varies the animation. It is not used for keys or real seed phrases.
struct DemoRng(u64);

impl DemoRng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        self.0
    }

    fn result(&mut self, label: &str) -> DemoResult {
        let tokens = (0..12)
            .map(|_| format!("demo-{:04}", self.next() % 10_000))
            .collect::<Vec<_>>()
            .join(" ");
        DemoResult {
            label: label.to_owned(),
            tokens,
            satoshis: self.next() % 50_000_000 + 1,
        }
    }
}

fn result_text(result: &DemoResult) -> String {
    format!(
        "[ДЕМО-РЕЗУЛЬТАТ] {}\n  Учебный набор (не BIP39): {}\n  Вымышленный баланс: {}.{:08} BTC\n",
        result.label,
        result.tokens,
        result.satoshis / 100_000_000,
        result.satoshis % 100_000_000,
    )
}

fn simulate(
    output: &mut impl Write,
    wallets: &[String],
    options: &Options,
    plain: bool,
) -> io::Result<String> {
    writeln!(output, "\nSEED DEMO | {DEMO_BANNER}")?;
    writeln!(
        output,
        "Локальная анимация: без проверки блокчейна и доступа к средствам."
    )?;
    writeln!(
        output,
        "Учебные токены demo-xxxx не являются seed-фразами BIP39."
    )?;
    writeln!(
        output,
        "Метки из списка: {} | Длительность: {} с",
        wallets.len(),
        options.seconds
    )?;
    for wallet in wallets.iter().take(5) {
        writeln!(output, "  {wallet}")?;
    }
    if wallets.len() > 5 {
        writeln!(output, "  ... и ещё {}", wallets.len() - 5)?;
    }
    writeln!(output)?;
    output.flush()?;

    let mut rng = DemoRng(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64,
    );
    let start = Instant::now();
    let duration = Duration::from_secs(options.seconds);
    let mut results = Vec::new();
    let result_count = wallets.len().min(3);
    let mut previous_second = None;
    let mut previous_width = 0;
    let mut frame = 0;

    loop {
        let elapsed = start.elapsed().min(duration);
        let fraction = elapsed.as_secs_f64() / duration.as_secs_f64();
        let due = ((fraction * (result_count + 1) as f64) as usize).min(result_count);
        while results.len() < due {
            if !plain && previous_width > 0 {
                write!(output, "\r{:width$}\r", "", width = previous_width)?;
                previous_width = 0;
            }
            let result = rng.result(&wallets[results.len()]);
            writeln!(output, "{}", result_text(&result))?;
            results.push(result);
        }

        if !plain || previous_second != Some(elapsed.as_secs()) || elapsed == duration {
            let completed = (fraction * 20.0) as usize;
            let bar = format!("{}{}", "#".repeat(completed), ".".repeat(20 - completed));
            let count = (elapsed.as_secs_f64() * DEMO_RATE as f64) as u64;
            let spinner = ['|', '/', '-', '\\'][frame % 4];
            let line = format!(
                "[ДЕМО] {spinner} [{bar}] {:3.0}% | {count:9} усл. попыток | {DEMO_RATE}/с",
                fraction * 100.0,
            );
            if plain {
                writeln!(output, "{line}")?;
            } else {
                let width = line.chars().count();
                write!(
                    output,
                    "\r{line}{}",
                    " ".repeat(previous_width.saturating_sub(width))
                )?;
                previous_width = width;
            }
            output.flush()?;
            previous_second = Some(elapsed.as_secs());
        }
        if elapsed == duration {
            break;
        }
        frame += 1;
        thread::sleep(FRAME_TIME.min(duration.saturating_sub(start.elapsed())));
    }
    if !plain {
        writeln!(output)?;
    }
    writeln!(
        output,
        "\nДемонстрация завершена. Вымышленных результатов: {}.",
        results.len()
    )?;
    writeln!(output, "{DEMO_BANNER}")?;

    let mut summary = format!(
        "SEED DEMO\n{DEMO_BANNER}\nНикакие кошельки, seed-фразы и балансы не проверялись.\n\nМетки: {}\nДлительность: {} с\nУсловных попыток: {}\n\n",
        wallets.len(), options.seconds, options.seconds * DEMO_RATE,
    );
    for result in &results {
        summary.push_str(&result_text(result));
        summary.push('\n');
    }
    Ok(summary)
}
