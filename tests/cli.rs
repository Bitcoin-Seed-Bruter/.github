use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempDir {
    path: PathBuf,
    root: PathBuf,
}

impl TempDir {
    fn new() -> Self {
        let root = fs::canonicalize(std::env::temp_dir()).expect("resolve temporary directory");
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock is after Unix epoch")
            .as_nanos();
        let id = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = root.join(format!(
            "seed-demo-cli-tests-{}-{timestamp}-{id}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create unique test directory");
        Self { path, root }
    }

    fn file(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        // Resolve and constrain the recursive cleanup to this test's temp directory.
        if let Ok(path) = fs::canonicalize(&self.path) {
            let has_test_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("seed-demo-cli-tests-"));
            if path.starts_with(&self.root) && path != self.root && has_test_name {
                let _ = fs::remove_dir_all(path);
            }
        }
    }
}

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_seed-demo"))
}

fn stdout(output: &Output) -> &str {
    std::str::from_utf8(&output.stdout).expect("stdout is UTF-8")
}

fn stderr(output: &Output) -> &str {
    std::str::from_utf8(&output.stderr).expect("stderr is UTF-8")
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "status: {}; stderr: {}; stdout: {}",
        output.status,
        stderr(output),
        stdout(output)
    );
}

fn run_with_wallets(path: &Path) -> Output {
    cli()
        .args(["--seconds", "1", "--plain", "--wallets"])
        .arg(path)
        .output()
        .expect("run seed-demo")
}

#[test]
fn help_identifies_the_program_as_a_demonstration() {
    let output = cli().arg("--help").output().expect("run --help");
    assert_success(&output);
    assert!(stdout(&output).contains("ДЕМОНСТРАЦИОННЫЙ"));
    assert!(stdout(&output).contains("результаты вымышлены"));
    assert!(stdout(&output).contains("--wallets"));
    assert!(stdout(&output).contains("--report"));
}

#[test]
fn invalid_duration_and_missing_values_are_rejected() {
    for args in [
        vec!["--seconds", "0"],
        vec!["--seconds", "3601"],
        vec!["--seconds", "-1"],
        vec!["--seconds", "1.5"],
        vec!["--seconds", "nope"],
        vec!["--seconds"],
        vec!["--seconds", "--plain"],
        vec!["--wallets"],
        vec!["--report"],
    ] {
        let output = cli().args(&args).output().expect("run invalid arguments");
        assert!(!output.status.success(), "accepted arguments: {args:?}");
        assert!(stderr(&output).contains("Ошибка"), "{args:?}");
        assert!(output.stdout.is_empty(), "started simulation for {args:?}");
    }
}

#[test]
fn utf8_bom_comments_and_duplicates_produce_two_fictional_results() {
    let temp = TempDir::new();
    let wallets = temp.file("метки.txt");
    fs::write(
        &wallets,
        "\u{feff}# Demo list\r\n\r\n  DEMO-ALPHA  \r\nDEMO-БЕТА\r\nDEMO-ALPHA\r\n  # ignored\r\n",
    )
    .expect("write wallet labels");

    let output = run_with_wallets(&wallets);
    assert_success(&output);
    let text = stdout(&output);
    assert!(text.contains("Метки из списка: 2"));
    assert!(text.contains("Вымышленных результатов: 2"));
    assert_eq!(text.matches("[ДЕМО-РЕЗУЛЬТАТ] DEMO-ALPHA").count(), 1);
    assert_eq!(text.matches("[ДЕМО-РЕЗУЛЬТАТ] DEMO-БЕТА").count(), 1);
    assert_eq!(text.matches("Вымышленный баланс:").count(), 2);
    assert!(
        !text.contains('\r'),
        "plain output contains carriage returns"
    );
    assert!(
        !text.contains('\u{1b}'),
        "plain output contains ANSI escapes"
    );

    let token_sets: Vec<_> = text
        .lines()
        .filter_map(|line| line.trim().strip_prefix("Учебный набор (не BIP39): "))
        .collect();
    assert_eq!(token_sets.len(), 2);
    for tokens in token_sets {
        let tokens: Vec<_> = tokens.split_whitespace().collect();
        assert_eq!(tokens.len(), 12);
        assert!(tokens.iter().all(|token| token.starts_with("demo-")));
    }
}

#[test]
fn report_is_marked_as_demonstration_and_cannot_be_overwritten() {
    let temp = TempDir::new();
    let report = temp.file("session.demo-report.txt");
    let output = cli()
        .args(["--seconds", "1", "--plain", "--report"])
        .arg(&report)
        .output()
        .expect("run with report");
    assert_success(&output);

    let original = fs::read(&report).expect("read created report");
    let text = std::str::from_utf8(&original).expect("report is UTF-8");
    assert!(text.contains("ДЕМОНСТРАЦИОННЫЙ РЕЖИМ"));
    assert!(text.contains("все результаты вымышлены"));
    assert!(text.contains("Никакие кошельки, seed-фразы и балансы не проверялись"));
    assert!(text.contains("[ДЕМО-РЕЗУЛЬТАТ]"));
    assert!(!text.contains("Незавершённый запуск"));

    let second = cli()
        .args(["--seconds", "1", "--plain", "--report"])
        .arg(&report)
        .output()
        .expect("run with existing report");
    assert!(!second.status.success());
    assert!(stderr(&second).contains("не удалось создать отчёт"));
    assert!(
        second.stdout.is_empty(),
        "simulation started despite existing report"
    );
    assert_eq!(fs::read(&report).expect("reread report"), original);
}

#[test]
fn empty_lists_and_control_characters_are_rejected() {
    let temp = TempDir::new();
    let wallets = temp.file("wallets.txt");
    for content in [
        "\u{feff}\n # comments only\n\n",
        "DEMO-\u{1b}[31mBAD\n",
        "DEMO-\0BAD\n",
    ] {
        fs::write(&wallets, content).expect("write invalid list");
        let output = run_with_wallets(&wallets);
        assert!(!output.status.success(), "accepted list: {content:?}");
        assert!(stderr(&output).contains("Ошибка"));
        assert!(
            output.stdout.is_empty(),
            "started simulation for invalid list"
        );
        assert!(!stderr(&output).contains('\u{1b}'));
    }
}
