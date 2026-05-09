use std::fs::File;
use std::io::{BufRead, IsTerminal, Write};
use std::env;
use std::path::PathBuf;

use monadb::{MonaDB, Result};
use rustyline::{error::ReadlineError, history::DefaultHistory, validate::{ValidationContext, ValidationResult, Validator}, Completer, Config, EditMode, Editor, Helper, Highlighter, Hinter};

use clap::{Parser, Subcommand};
use termcolor::StandardStream;
use termcolor::{Color, ColorChoice, ColorSpec,WriteColor};

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None, multicall = true)]
struct Cli {}

#[derive(Debug, Parser)]
#[command(multicall = true)]
struct Commands {
    #[command(subcommand)]
    command: Command
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Describe the catalog.
    Info,
    /// Toggle debug mode.
    Debug,
    /// Exit the shell.
    Exit,
}

pub struct LineReader {
    editor: Editor<LineValidator, DefaultHistory>,
}

impl LineReader {

    pub fn close(&mut self) {
        self.editor.save_history(".monadb_history").unwrap();
    }

    pub fn read_line(&mut self, buffer: &mut String, prompt: &str) -> Option<()> {
        let readline = self.editor.readline(prompt);
        match readline {
            Ok(line) => {
                let _ = self.editor.add_history_entry(line.as_str());
                *buffer = line;
                Some(())
            }
            Err(ReadlineError::Interrupted) => {
                // CTRL-C
                None
            }
            Err(ReadlineError::Eof) => {
                // CTRL-D
                None
            }
            Err(_) => None,
        }
    }
}

impl Default for LineReader {

    fn default() -> Self {
        let config = Config::builder()
            .edit_mode(EditMode::Vi)
            .build();
        let mut editor = Editor::<LineValidator, DefaultHistory>::with_config(config).unwrap();
        let path: PathBuf = ".monadb_history".into();
        _ = File::create_new(&path); // touch
        editor.load_history(&path).unwrap();
        editor.set_helper(Some(LineValidator));
        LineReader { editor }
    }
}

#[derive(Helper, Highlighter, Hinter, Completer)]
struct LineValidator;

impl Validator for LineValidator {
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

fn main() {

    // TODO allow executing as a command `cat file.jsonl | mona -q 'select * from stdin'`
    let input = std::io::stdin();
    if !input.is_terminal() {
        for line in input.lock().lines() {
            println!("{}", line.unwrap())
        }
        return;
    }

    // OPEN DATABASE 
    let args: Vec<String> = env::args().collect();
    let db = match args.len() {
        1 => {
            MonaDB::memory().expect("Could not open MonaDB memory")
        },
        2 => {
            let path: PathBuf = PathBuf::from(&args[1]);
            MonaDB::open(&path).expect("Could not open MonaDB")
        },
        _ => {
            eprintln!("Usage: {} <file>", args[0]);
            std::process::exit(1);
        },
    };

    // INIT REPL 
    let mut app = App::new(db);
    let mut line_reader = LineReader::default();
    let mut buffer = String::new();

    // MAIN LOOP
    loop {
        if line_reader.read_line(&mut buffer, ">> ").is_none() {
            // EOF (or interrupt)
            break;
        }
        if buffer.is_empty() {
            // WHITESPACE
            continue;
        }
        if buffer.starts_with(".") {
            // COMMAND
            let line = buffer.strip_prefix(".").unwrap();
            let args = shlex::split(line).unwrap();
            let command = match Commands::try_parse_from(&args) {
                Ok(commands) => commands.command,
                Err(message) => {
                    eprintln!("{}", message);
                    continue;
                },
            };
            // EXECUTE COMMAND
            match command {
                Command::Debug => app.debug(),
                Command::Info => app.info(),
                Command::Exit => break,
            }.unwrap();
        } else {
            // STATEMENT
            app.exec(&buffer).unwrap();
        }
    }

    // CLEANUP
    line_reader.close();
}

struct App {
    db: MonaDB,
    debug: bool,
    out: StandardStream,
}

impl App {

    pub fn new(db: MonaDB) -> Self {
        App { 
            db,
            debug: false,
            out: StandardStream::stdout(ColorChoice::Always),
        }
    }

    pub fn debug(&mut self) -> Result<()> {
        self.debug = !self.debug;
        println!("debug: {}", self.debug);
        Ok(())
    }

    pub fn info(&mut self) -> Result<()> {
        self.exec("select * from catalog;\n")
    }

    pub fn exec(&mut self, statement: &str) -> Result<()> {
        match self.db.exec(statement, self.debug) {
            Ok(mut rows) => {
                loop {
                    match rows.next() {
                        Ok(Some(row)) => println!("{:?}", row),
                        Ok(None) => break,
                        Err(e) => {
                            // runtime error
                            println!("{:?}", e);
                            println!("error.");
                        }
                    }
                }
                // remove these additional lines?
                println!("ok.");
            },
            Err(e) => {
                self.out.set_color(ColorSpec::new().set_fg(Some(Color::Red))).unwrap();
                writeln!(self.out, "{}", e.pretty(statement)).unwrap();
                self.out.reset().unwrap();
                writeln!(self.out).unwrap();
            }
        };
        Ok(())
    }
}
