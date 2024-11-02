use std::env;
use std::path::PathBuf;

use monadb::{value::Value, MonaDB};
use rustyline::{error::ReadlineError, history::DefaultHistory, validate::{ValidationContext, ValidationResult, Validator}, Completer, Config, EditMode, Editor, Helper, Highlighter, Hinter};

use clap::{Parser, Subcommand};

/// Cli is the command line interface for rho, but nothing is here yet..
#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None, multicall = true)]
struct Cli {}

/// TODO
#[derive(Debug, Parser)]
#[command(multicall = true)]
struct Commands {
    #[command(subcommand)]
    command: Command
}

/// TODO
#[derive(Debug, Subcommand)]
enum Command {
    /// Describe the catalog.
    Info,
    /// Toggle debug mode.
    Debug,
    /// TODO TEMPORARY – Create a table.
    Create {
        table: String,
    },
    /// TODO TEMPORARY – Insert a row.
    Insert {
        table: String,
        value: String,
    },
    /// TODO TEMPORARY – Drop a table.
    Drop {
        table: String,
    },
    /// TODO TEMPORARY – Select from a table.
    Select {
        table: String,
    },
    /// Exit the shell.
    Exit,
}

pub struct LineReader {
    editor: Editor<LineValidator, DefaultHistory>,
}

impl LineReader {
    pub fn new() -> LineReader {
        let config = Config::builder()
            .edit_mode(EditMode::Vi)
            .build();
        let mut editor = Editor::<LineValidator, DefaultHistory>::with_config(config).unwrap();
        // todo check if history file exists
        editor.load_history(".monadb_history").unwrap();
        editor.set_helper(Some(LineValidator));
        LineReader { editor }
    }

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

    // OPEN DATABASE 
    let args: Vec<String> = env::args().collect();
    let mut mona = match args.len() {
        1 => {
            MonaDB::memory().expect("Could not open monadb memory")
        },
        2 => {
            let path: PathBuf = PathBuf::from(&args[1]);
            MonaDB::open(&path).expect("Could not open monadb file")
        },
        _ => {
            eprintln!("Usage: {} <file>", args[0]);
            std::process::exit(1);
        },
    };

    // REPL 
    let mut line_reader = LineReader::new();
    let mut buffer = String::new();
    let mut debug = false;

    // MAIN LOOP
    loop {
        if line_reader.read_line(&mut buffer, ">> ") == None {
            // EOF or interrupt
            break;
        }
        if buffer.is_empty() {
            continue;
        }
        if buffer.starts_with(".") {
            // PARSE COMMAND
            let line = buffer.strip_prefix(".").unwrap();
            let args = shlex::split(&line).unwrap();
            let command = match Commands::try_parse_from(&args) {
                Ok(commands) => commands.command,
                Err(message) => {
                    eprintln!("{}", message);
                    continue;
                },
            };
            // EXECUTE COMMAND
            match command {
                Command::Debug => {
                    debug = !debug;
                    println!("debug: {}", debug);
                }
                Command::Info => {
                    mona.info();
                }
                Command::Exit => {
                    break;
                }
                Command::Create { table } => {
                    // this would require table being public
                    // what can I do differently?
                    todo!("create table {}", table);
                },
                Command::Insert { table, value } => {
                    let row = Value::from_str(&value).expect("Could not parse row");
                    mona.insert(&table, row).expect("Could not insert row");
                },
                Command::Drop { table } => {
                    mona.drop_table(&table).expect("Could not drop table");
                },
                Command::Select { table } => {
                    let mut cursor = mona.scan(&table).expect("Could not select rows");
                    if !cursor.rewind() {
                        break; // empty
                    }
                    while {
                        let row = cursor.curr();
                        println!("{:?}", row.val);
                        cursor.next()
                    } {}
                },
            }
        } else {
            // STATEMENT
            match mona.exec(&buffer, debug) {
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
                    // compilation error
                    println!("{:?}", e);
                    println!("error.");
                }
            }
        }
    }

    // CLEANUP
    line_reader.close();
}
