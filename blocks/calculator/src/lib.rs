//! Stateful expression calculator exposed as an AIOS WASM store block.
//!
//! The console sends the expression text as a `Payload::Custom` request; the
//! block keeps `ans`, user variables and a short history in guest memory.

use aios_block_sdk::Request;
use std::sync::{Mutex, MutexGuard, OnceLock};

const HELP: &str = "calculator: <expression> | <name> = <expression> | ans | history | reset | help\noperators: + - * / % ( )   variables: ans and assigned names";

#[derive(Default)]
struct CalcState {
    ans: f64,
    history: Vec<String>,
    vars: Vec<(String, f64)>,
}

static STATE: OnceLock<Mutex<CalcState>> = OnceLock::new();

fn state() -> MutexGuard<'static, CalcState> {
    STATE
        .get_or_init(|| Mutex::new(CalcState::default()))
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

struct Parser<'a> {
    input: &'a [u8],
    pos: usize,
    ans: f64,
    vars: &'a [(String, f64)],
}

impl<'a> Parser<'a> {
    fn new(input: &'a [u8], ans: f64, vars: &'a [(String, f64)]) -> Self {
        Self {
            input,
            pos: 0,
            ans,
            vars,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let byte = self.peek();
        if byte.is_some() {
            self.pos += 1;
        }
        byte
    }

    fn skip_ws(&mut self) {
        while self.peek().is_some_and(|byte| byte.is_ascii_whitespace()) {
            self.pos += 1;
        }
    }

    fn parse(&mut self) -> Result<f64, String> {
        let value = self.parse_expr()?;
        self.skip_ws();
        if self.peek().is_some() {
            return Err(format!("unexpected input at byte {}", self.pos));
        }
        Ok(value)
    }

    fn parse_expr(&mut self) -> Result<f64, String> {
        let mut value = self.parse_term()?;
        loop {
            self.skip_ws();
            match self.peek() {
                Some(b'+') => {
                    self.bump();
                    value += self.parse_term()?;
                }
                Some(b'-') => {
                    self.bump();
                    value -= self.parse_term()?;
                }
                _ => return Ok(value),
            }
        }
    }

    fn parse_term(&mut self) -> Result<f64, String> {
        let mut value = self.parse_unary()?;
        loop {
            self.skip_ws();
            match self.peek() {
                Some(b'*') => {
                    self.bump();
                    value *= self.parse_unary()?;
                }
                Some(b'/') => {
                    self.bump();
                    let divisor = self.parse_unary()?;
                    if divisor == 0.0 {
                        return Err("division by zero".into());
                    }
                    value /= divisor;
                }
                Some(b'%') => {
                    self.bump();
                    let divisor = self.parse_unary()?;
                    if divisor == 0.0 {
                        return Err("division by zero".into());
                    }
                    value %= divisor;
                }
                _ => return Ok(value),
            }
        }
    }

    fn parse_unary(&mut self) -> Result<f64, String> {
        self.skip_ws();
        match self.peek() {
            Some(b'+') => {
                self.bump();
                self.parse_unary()
            }
            Some(b'-') => {
                self.bump();
                Ok(-self.parse_unary()?)
            }
            _ => self.parse_primary(),
        }
    }

    fn parse_primary(&mut self) -> Result<f64, String> {
        self.skip_ws();
        match self.peek() {
            Some(b'(') => {
                self.bump();
                let value = self.parse_expr()?;
                self.skip_ws();
                if self.bump() != Some(b')') {
                    return Err("missing closing parenthesis".into());
                }
                Ok(value)
            }
            Some(byte) if byte.is_ascii_digit() || byte == b'.' => self.parse_number(),
            Some(byte) if byte.is_ascii_alphabetic() || byte == b'_' => self.parse_variable(),
            Some(byte) => Err(format!("unexpected character '{}'", byte as char)),
            None => Err("missing value".into()),
        }
    }

    fn parse_number(&mut self) -> Result<f64, String> {
        let start = self.pos;
        let mut dots = 0;
        while let Some(byte) = self.peek() {
            if byte.is_ascii_digit() {
                self.bump();
            } else if byte == b'.' {
                dots += 1;
                if dots > 1 {
                    return Err("invalid number".into());
                }
                self.bump();
            } else {
                break;
            }
        }
        let text =
            std::str::from_utf8(&self.input[start..self.pos]).map_err(|_| "invalid number")?;
        text.parse::<f64>().map_err(|_| "invalid number".into())
    }

    fn parse_variable(&mut self) -> Result<f64, String> {
        let start = self.pos;
        while let Some(byte) = self.peek() {
            if byte.is_ascii_alphanumeric() || byte == b'_' {
                self.bump();
            } else {
                break;
            }
        }
        let name = &self.input[start..self.pos];
        if name.eq_ignore_ascii_case(b"ans") {
            return Ok(self.ans);
        }
        self.vars
            .iter()
            .find(|(key, _)| key.as_bytes().eq_ignore_ascii_case(name))
            .map(|(_, value)| *value)
            .ok_or_else(|| format!("unknown variable '{}'", String::from_utf8_lossy(name)))
    }
}

fn is_identifier(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && (bytes[0].is_ascii_alphabetic() || bytes[0] == b'_')
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
}

fn format_number(value: f64) -> String {
    if value.is_nan() {
        return "NaN".into();
    }
    if value.is_infinite() {
        return if value.is_sign_negative() {
            "-inf"
        } else {
            "inf"
        }
        .into();
    }
    if value.fract().abs() < 1e-12 && value.abs() < 9e15 {
        return format!("{}", value as i64);
    }
    let text = format!("{value:.10}");
    let trimmed = text.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() { "0" } else { trimmed }.to_string()
}

/// Handles one calculator request and returns the text reply for the host.
pub fn handle(request: &Request) -> String {
    let raw = request.text.trim();
    if raw.is_empty() {
        return HELP.to_string();
    }
    let lower = raw.to_ascii_lowercase();
    if lower == "help" || lower == "?" {
        return HELP.to_string();
    }
    let mut current = state();
    if lower == "reset" {
        current.ans = 0.0;
        current.history.clear();
        current.vars.clear();
        return "calculator state reset".into();
    }
    if lower == "history" {
        if current.history.is_empty() {
            return "no calculations yet".into();
        }
        return current.history.join("\n");
    }
    if lower == "ans" {
        return format_number(current.ans);
    }

    let (target, expression) = match raw.split_once('=') {
        Some((name, expression)) if is_identifier(name.trim()) => {
            (Some(name.trim().to_ascii_lowercase()), expression.trim())
        }
        _ => (None, raw),
    };
    let value = {
        let mut parser = Parser::new(expression.as_bytes(), current.ans, &current.vars);
        match parser.parse() {
            Ok(value) => value,
            Err(error) => return format!("error: {error}"),
        }
    };
    let rendered = format_number(value);
    current.ans = value;
    current.history.push(format!("{expression} = {rendered}"));
    if current.history.len() > 20 {
        current.history.remove(0);
    }
    if let Some(name) = target {
        current.vars.retain(|(key, _)| key != &name);
        current.vars.push((name.clone(), value));
        format!("{name} = {rendered}")
    } else {
        rendered
    }
}

/// Host ABI entry point exported by the compiled WASM block.
#[no_mangle]
pub extern "C" fn handle_message(ptr: i32, len: i32) -> i64 {
    aios_block_sdk::process_request(ptr, len, handle)
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn request(text: &str) -> Request {
        Request {
            tag: "calculator".into(),
            text: text.into(),
        }
    }

    #[test]
    fn evaluates_precedence_and_parentheses() {
        let _guard = TEST_LOCK.lock().unwrap();
        handle(&request("reset"));
        assert_eq!(handle(&request("2+3*4")), "14");
        assert_eq!(handle(&request("(2+3)*4")), "20");
        assert_eq!(handle(&request("-8 / 2 + 1")), "-3");
    }

    #[test]
    fn keeps_answer_and_variables_between_requests() {
        let _guard = TEST_LOCK.lock().unwrap();
        handle(&request("reset"));
        assert_eq!(handle(&request("6*7")), "42");
        assert_eq!(handle(&request("ans + 1")), "43");
        assert_eq!(handle(&request("width = 3")), "width = 3");
        assert_eq!(handle(&request("width * width")), "9");
    }

    #[test]
    fn reports_domain_errors() {
        let _guard = TEST_LOCK.lock().unwrap();
        handle(&request("reset"));
        assert_eq!(handle(&request("1 / 0")), "error: division by zero");
        assert!(handle(&request("2 +")).starts_with("error:"));
    }

    #[test]
    fn help_and_history_are_text_commands() {
        let _guard = TEST_LOCK.lock().unwrap();
        handle(&request("reset"));
        assert!(handle(&request("help")).contains("operators"));
        assert_eq!(handle(&request("history")), "no calculations yet");
        handle(&request("1+1"));
        assert_eq!(handle(&request("history")), "1+1 = 2");
    }
}
