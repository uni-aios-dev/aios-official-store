//! Stateful console minesweeper exposed as an AIOS WASM store block.
//!
//! Board state lives in guest memory for the lifetime of the instantiated
//! block, so consecutive `block run minesweeper ...` commands continue a game.

use aios_block_sdk::Request;
use std::sync::{Mutex, MutexGuard, OnceLock};

const HELP: &str =
    "minesweeper: new [w h mines] | open <x> <y> | flag <x> <y> | show | status | reset | help
coordinates are 1-based (row and column); open starts a default 9x9/10 game if needed";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Playing,
    Won,
    Lost,
}

struct Game {
    width: usize,
    height: usize,
    mines: Vec<bool>,
    revealed: Vec<bool>,
    flagged: Vec<bool>,
    phase: Phase,
}

static GAME: OnceLock<Mutex<Option<Game>>> = OnceLock::new();

fn game() -> MutexGuard<'static, Option<Game>> {
    GAME.get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

fn next_random(seed: &mut u64) -> u64 {
    *seed = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    *seed >> 33
}

fn create_game(
    width: usize,
    height: usize,
    mine_count: usize,
    first_safe: Option<usize>,
) -> Result<Game, String> {
    let cells = width * height;
    if !(2..=30).contains(&width) || !(2..=30).contains(&height) {
        return Err("width and height must be between 2 and 30".into());
    }
    if mine_count == 0 || mine_count >= cells {
        return Err(format!("mine count must be between 1 and {}", cells - 1));
    }
    let mut seed = 0x9E37_79B9_7F4A_7C15
        ^ (width as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9)
        ^ (height as u64).wrapping_mul(0x94D0_49BB_1331_11EB)
        ^ (mine_count as u64).wrapping_mul(0x2545_F491_4F6C_DD1D);
    let mut mines = vec![false; cells];
    let mut placed = 0usize;
    let mut guard = 0usize;
    while placed < mine_count {
        let index = (next_random(&mut seed) as usize) % cells;
        if Some(index) == first_safe || mines[index] {
            guard += 1;
            if guard > cells * 100 {
                return Err("could not place mines".into());
            }
            continue;
        }
        mines[index] = true;
        placed += 1;
    }
    Ok(Game {
        width,
        height,
        mines,
        revealed: vec![false; cells],
        flagged: vec![false; cells],
        phase: Phase::Playing,
    })
}

impl Game {
    fn index(&self, x: usize, y: usize) -> Option<usize> {
        if x == 0 || y == 0 || x > self.width || y > self.height {
            None
        } else {
            Some((y - 1) * self.width + (x - 1))
        }
    }

    fn neighbors(&self, index: usize) -> Vec<usize> {
        let x = index % self.width;
        let y = index / self.width;
        let mut result = Vec::new();
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx >= 0 && ny >= 0 && (nx as usize) < self.width && (ny as usize) < self.height {
                    result.push(ny as usize * self.width + nx as usize);
                }
            }
        }
        result
    }

    fn adjacent_mines(&self, index: usize) -> usize {
        self.neighbors(index)
            .iter()
            .filter(|neighbor| self.mines[**neighbor])
            .count()
    }

    fn reveal(&mut self, index: usize) {
        let mut stack = vec![index];
        while let Some(current) = stack.pop() {
            if self.revealed[current] || self.flagged[current] || self.mines[current] {
                continue;
            }
            self.revealed[current] = true;
            if self.adjacent_mines(current) == 0 {
                for neighbor in self.neighbors(current) {
                    if !self.revealed[neighbor] && !self.flagged[neighbor] && !self.mines[neighbor]
                    {
                        stack.push(neighbor);
                    }
                }
            }
        }
    }

    fn check_won(&mut self) {
        let all_safe_revealed = self
            .mines
            .iter()
            .enumerate()
            .all(|(index, mine)| *mine || self.revealed[index]);
        if all_safe_revealed {
            self.phase = Phase::Won;
            for index in 0..self.mines.len() {
                if self.mines[index] {
                    self.flagged[index] = true;
                }
            }
        }
    }

    fn render(&self) -> String {
        let mut lines = Vec::new();
        let header: String = (1..=self.width)
            .map(|column| format!("{:>2}", column))
            .collect();
        lines.push(format!("   {header}"));
        for row in 1..=self.height {
            let mut line = format!("{row:>2} ");
            for column in 1..=self.width {
                let index = self.index(column, row).expect("loop bounds");
                let cell = if self.flagged[index] {
                    "F"
                } else if !self.revealed[index] {
                    "."
                } else if self.mines[index] {
                    "*"
                } else {
                    match self.adjacent_mines(index) {
                        0 => "-",
                        count => return_cell_digit(count),
                    }
                };
                line.push_str(&format!("{cell:>2}"));
            }
            lines.push(line);
        }
        lines.join("\n")
    }

    fn status(&self) -> &'static str {
        match self.phase {
            Phase::Playing => "playing",
            Phase::Won => "won",
            Phase::Lost => "lost",
        }
    }
}

fn return_cell_digit(count: usize) -> &'static str {
    match count {
        1 => "1",
        2 => "2",
        3 => "3",
        4 => "4",
        5 => "5",
        6 => "6",
        7 => "7",
        8 => "8",
        _ => "*",
    }
}

fn parse_coordinates(args: &[&str]) -> Result<(usize, usize), String> {
    if args.len() != 2 {
        return Err("expected x y".into());
    }
    let x = args[0]
        .parse::<usize>()
        .map_err(|_| format!("invalid x '{}'", args[0]))?;
    let y = args[1]
        .parse::<usize>()
        .map_err(|_| format!("invalid y '{}'", args[1]))?;
    Ok((x, y))
}

/// Handles one minesweeper request and returns the text reply for the host.
pub fn handle(request: &Request) -> String {
    let trimmed = request.text.trim();
    let mut words = trimmed.split_whitespace();
    let command = words.next().unwrap_or("help").to_ascii_lowercase();
    let args: Vec<&str> = words.collect();
    if command == "help" || command == "?" || trimmed.is_empty() {
        return HELP.to_string();
    }

    let mut current = game();
    match command.as_str() {
        "new" => {
            let numbers = if args.is_empty() {
                Ok(vec![9usize, 9, 10])
            } else if args.len() == 3 {
                args.iter()
                    .map(|value| value.parse::<usize>())
                    .collect::<Result<Vec<_>, _>>()
            } else {
                return "error: usage: new [w h mines]".into();
            };
            let numbers = match numbers {
                Ok(numbers) => numbers,
                Err(_) => return "error: width, height and mines must be integers".into(),
            };
            match create_game(numbers[0], numbers[1], numbers[2], None) {
                Ok(board) => {
                    *current = Some(board);
                    format!(
                        "new board {}x{} with {} mines\n{}",
                        numbers[0],
                        numbers[1],
                        numbers[2],
                        current.as_ref().expect("board").render()
                    )
                }
                Err(error) => format!("error: {error}"),
            }
        }
        "reset" => match create_game(9, 9, 10, None) {
            Ok(board) => {
                *current = Some(board);
                format!(
                    "new board 9x9 with 10 mines\n{}",
                    current.as_ref().expect("board").render()
                )
            }
            Err(error) => format!("error: {error}"),
        },
        "show" | "status" => match current.as_ref() {
            Some(board) => format!("{}\nstatus: {}", board.render(), board.status()),
            None => "no game; use `new [w h mines]` or `open x y`".into(),
        },
        "open" => {
            let (x, y) = match parse_coordinates(&args) {
                Ok(coordinates) => coordinates,
                Err(error) => return format!("error: {error}"),
            };
            if current.is_none() {
                let first = Some((y.saturating_sub(1)) * 9 + x.saturating_sub(1));
                match create_game(9, 9, 10, first) {
                    Ok(board) => *current = Some(board),
                    Err(error) => return format!("error: {error}"),
                }
            }
            let board = current.as_mut().expect("board");
            let Some(index) = board.index(x, y) else {
                return format!(
                    "error: coordinates out of range (board {}x{})",
                    board.width, board.height
                );
            };
            if board.phase != Phase::Playing {
                return format!("{}\nstatus: {}", board.render(), board.status());
            }
            if board.revealed[index] {
                return "already revealed".into();
            }
            if board.mines[index] {
                board.phase = Phase::Lost;
                for cell in 0..board.mines.len() {
                    if board.mines[cell] {
                        board.revealed[cell] = true;
                    }
                }
                return format!("BOOM!\n{}\nstatus: lost", board.render());
            }
            board.reveal(index);
            board.check_won();
            format!("{}\nstatus: {}", board.render(), board.status())
        }
        "flag" => {
            let (x, y) = match parse_coordinates(&args) {
                Ok(coordinates) => coordinates,
                Err(error) => return format!("error: {error}"),
            };
            let Some(board) = current.as_mut() else {
                return "no game; use `new [w h mines]` first".into();
            };
            let Some(index) = board.index(x, y) else {
                return format!(
                    "error: coordinates out of range (board {}x{})",
                    board.width, board.height
                );
            };
            if board.revealed[index] {
                return "cannot flag a revealed cell".into();
            }
            board.flagged[index] = !board.flagged[index];
            format!("{}\nstatus: {}", board.render(), board.status())
        }
        other => format!("error: unknown command '{other}'; use `help`"),
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
            tag: "minesweeper".into(),
            text: text.into(),
        }
    }

    #[test]
    fn new_board_and_show_render_grid() {
        let _guard = TEST_LOCK.lock().unwrap();
        let output = handle(&request("new 4 4 2"));
        assert!(output.contains("new board 4x4 with 2 mines"));
        let shown = handle(&request("show"));
        assert!(shown.contains("status: playing"));
        assert!(shown.contains("   1 2 3 4"));
    }

    #[test]
    fn open_first_cell_is_never_a_mine() {
        let _guard = TEST_LOCK.lock().unwrap();
        handle(&request("new 5 5 5"));
        let output = handle(&request("open 1 1"));
        assert!(!output.contains("BOOM"));
        assert!(output.contains("status: playing") || output.contains("status: won"));
    }

    #[test]
    fn flag_toggles_and_rejects_out_of_range() {
        let _guard = TEST_LOCK.lock().unwrap();
        handle(&request("new 4 4 1"));
        let output = handle(&request("flag 1 1"));
        assert!(output.contains("\n 1  F"));
        let output = handle(&request("flag 1 1"));
        assert!(output.contains("\n 1  ."));
        assert!(handle(&request("flag 9 9")).contains("out of range"));
    }

    #[test]
    fn help_lists_commands() {
        let _guard = TEST_LOCK.lock().unwrap();
        assert!(handle(&request("help")).contains("open <x> <y>"));
    }
}
