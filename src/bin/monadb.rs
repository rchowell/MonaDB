//! Interactive MonaDB shell (`monadb`).

use std::borrow::Cow;
use std::io::{BufRead, IsTerminal, Write};
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use monadb::highlight::highlight_line;
use monadb::MonaDB;
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::history::DefaultHistory;
use rustyline::validate::{ValidationContext, ValidationResult, Validator};
use rustyline::{Completer, Config, EditMode, Editor, Helper, Hinter};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

/// Top-level CLI arguments.
#[derive(Debug, Parser)]
#[command(
    name = "monadb",
    version,
    about = "Interactive MonaDB SQL shell",
    long_about = None
)]
struct Args {
    /// Database file path (default: in-memory).
    db: Option<PathBuf>,
}

#[derive(Debug, Parser)]
#[command(multicall = true)]
struct Commands {
    #[command(subcommand)]
    command: ShellCommand,
}

#[derive(Debug, Subcommand)]
enum ShellCommand {
    /// Describe the catalog.
    Info,
    /// Toggle debug mode.
    Debug,
    /// Exit the shell.
    Exit,
}

/// rustyline helper: syntax highlighting and multiline validation.
#[derive(Helper, Completer, Hinter)]
struct ReplHelper;

impl Highlighter for ReplHelper {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        if line.is_empty() {
            Cow::Borrowed(line)
        } else {
            Cow::Owned(highlight_line(line))
        }
    }
}

impl Validator for ReplHelper {
    fn validate(&self, ctx: &mut ValidationContext) -> rustyline::Result<ValidationResult> {
        let input = ctx.input();
        if input.starts_with('.') || input.ends_with(';') {
            Ok(ValidationResult::Valid(None))
        } else {
            Ok(ValidationResult::Incomplete)
        }
    }

    fn validate_while_typing(&self) -> bool {
        false
    }
}

struct LineReader {
    editor: Editor<ReplHelper, DefaultHistory>,
}

impl LineReader {
    fn close(&mut self) {
        let _ = self.editor.save_history(".monadb_history");
    }

    fn read_line(&mut self, buffer: &mut String, prompt: &str) -> Option<()> {
        match self.editor.readline(prompt) {
            Ok(line) => {
                let _ = self.editor.add_history_entry(line.as_str());
                *buffer = line;
                Some(())
            }
            Err(ReadlineError::Interrupted) => None,
            Err(ReadlineError::Eof) => None,
            Err(_) => None,
        }
    }
}

impl Default for LineReader {
    fn default() -> Self {
        let config = Config::builder().edit_mode(EditMode::Vi).build();
        let mut editor = Editor::<ReplHelper, DefaultHistory>::with_config(config)
            .expect("could not create line editor");
        let _ = editor.load_history(".monadb_history");
        editor.set_helper(Some(ReplHelper));
        Self { editor }
    }
}

fn print_error(stdout: &mut StandardStream, sql: &str, err: &monadb::error::Error) {
    stdout
        .set_color(ColorSpec::new().set_fg(Some(Color::Red)))
        .expect("set color");
    writeln!(stdout, "{}", err.pretty(sql)).expect("write error");
    stdout.reset().expect("reset color");
    writeln!(stdout).expect("write newline");
}

fn run_statement(db: &mut MonaDB, sql: &str, debug: bool, stdout: &mut StandardStream) {
    match db.query(sql, debug) {
        Ok(mut rows) => {
            let mut count = 0u64;
            loop {
                match rows.next() {
                    Ok(Some(row)) => {
                        println!("{row}");
                        count += 1;
                    }
                    Ok(None) => break,
                    Err(e) => {
                        print_error(stdout, sql, &e);
                        return;
                    }
                }
            }
            let mutations = rows.mutations();
            if count == 0 && mutations > 0 {
                println!("{mutations} row(s) affected");
            } else if count > 0 {
                println!();
            }
        }
        Err(e) => print_error(stdout, sql, &e),
    }
}

fn print_catalog(db: &mut MonaDB) {
    let sql = "select catalog.name, catalog.type, catalog.sql from catalog order by catalog.name;";
    let Ok(mut rows) = db.query(sql, false) else {
        return;
    };

    let mut entries: Vec<(String, String, String)> = Vec::new();
    while let Ok(Some(row)) = rows.next() {
        let name = row.jpk("name").map(|v| v.to_string()).unwrap_or_default();
        let kind = row.jpk("type").map(|v| v.to_string()).unwrap_or_default();
        let ddl = row.jpk("sql").map(|v| v.to_string()).unwrap_or_default();
        entries.push((name, kind, ddl));
    }

    if entries.is_empty() {
        println!("(empty catalog)");
        return;
    }

    let name_w = entries.iter().map(|(n, _, _)| n.len()).max().unwrap_or(4);
    let type_w = entries.iter().map(|(_, t, _)| t.len()).max().unwrap_or(4);
    println!(
        "{:<name_w$}  {:<type_w$}  sql",
        "name", "type", name_w = name_w, type_w = type_w
    );
    println!(
        "{}  {}  ---",
        "-".repeat(name_w),
        "-".repeat(type_w),
    );
    for (name, kind, ddl) in &entries {
        let sql_display = if ddl.len() > 60 {
            format!("{}…", &ddl[..57])
        } else {
            ddl.clone()
        };
        println!("{name:<name_w$}  {kind:<type_w$}  {sql_display}");
    }
    println!();
}

fn open_database(db: Option<PathBuf>) -> MonaDB {
    match db {
        None => MonaDB::memory().expect("could not open in-memory MonaDB"),
        Some(path) => MonaDB::open(&path).unwrap_or_else(|e| {
            eprintln!("could not open MonaDB at {}: {e:?}", path.display());
            std::process::exit(1);
        }),
    }
}

fn run_repl(db: &mut MonaDB) {
    let mut line_reader = LineReader::default();
    let mut buffer = String::new();
    let mut debug = false;
    let mut stdout = StandardStream::stdout(ColorChoice::Auto);

    loop {
        if line_reader.read_line(&mut buffer, ">> ").is_none() {
            break;
        }
        if buffer.is_empty() {
            continue;
        }

        if buffer.starts_with('.') {
            let line = buffer.strip_prefix('.').unwrap_or(&buffer);
            let args = shlex::split(line).unwrap_or_default();
            if args.is_empty() {
                continue;
            }
            let command = match Commands::try_parse_from(&args) {
                Ok(commands) => commands.command,
                Err(message) => {
                    eprintln!("{message}");
                    continue;
                }
            };
            match command {
                ShellCommand::Debug => {
                    debug = !debug;
                    println!("debug: {debug}");
                }
                ShellCommand::Info => print_catalog(db),
                ShellCommand::Exit => break,
            }
        } else {
            run_statement(db, &buffer, debug, &mut stdout);
        }
    }

    line_reader.close();
}

fn main() {
    let args = Args::parse();

    let stdin = std::io::stdin();
    if !stdin.is_terminal() {
        for line in stdin.lock().lines() {
            match line {
                Ok(line) => println!("{line}"),
                Err(e) => {
                    eprintln!("{e}");
                    std::process::exit(1);
                }
            }
        }
        return;
    }

    let mut db = open_database(args.db);
    run_repl(&mut db);
}
