use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::path::Path;

const MAX_BYTES: u64 = 1_048_576;
const MAX_WALLETS: usize = 10_000;

pub fn load(path: Option<&Path>) -> Result<Vec<String>, String> {
    let Some(path) = path else {
        return Ok((1..=3).map(|id| format!("DEMO-BTC-{id:03}")).collect());
    };
    let file = File::open(path)
        .map_err(|error| format!("не удалось открыть список {}: {error}", path.display()))?;
    let mut content = String::new();
    file.take(MAX_BYTES + 1)
        .read_to_string(&mut content)
        .map_err(|error| format!("не удалось прочитать UTF-8 список: {error}"))?;
    if content.len() as u64 > MAX_BYTES {
        return Err("список превышает 1 МиБ".into());
    }
    let mut seen = HashSet::new();
    let mut wallets = Vec::new();
    for (index, line) in content.trim_start_matches('\u{feff}').lines().enumerate() {
        let label = line.trim();
        if label.is_empty() || label.starts_with('#') {
            continue;
        }
        if label.chars().any(char::is_control) || label.chars().count() > 160 {
            return Err(format!(
                "строка {}: метка должна содержать до 160 печатных символов",
                index + 1
            ));
        }
        if seen.insert(label.to_owned()) {
            wallets.push(label.to_owned());
        }
        if wallets.len() > MAX_WALLETS {
            return Err("в списке больше 10 000 уникальных меток".into());
        }
    }
    if wallets.is_empty() {
        return Err("список пуст: добавьте хотя бы одну метку или адрес".into());
    }
    Ok(wallets)
}
