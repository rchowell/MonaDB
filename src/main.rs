use std::env;
use std::path::PathBuf;

use rho::{value::Value, Rho};
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
        editor.load_history(".rho_history").unwrap();
        editor.set_helper(Some(LineValidator));
        LineReader { editor }
    }

    pub fn close(&mut self) {
        self.editor.save_history(".rho_history").unwrap();
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
    let mut rho = match args.len() {
        1 => {
            Rho::memory().expect("Could not open rho")
        },
        2 => {
            let path: PathBuf = PathBuf::from(&args[1]);
            Rho::open(&path).expect("Could not open rho")
        },
        _ => {
            eprintln!("Usage: {} <file>", args[0]);
            std::process::exit(1);
        },
    };

    // REPL 
    let mut line_reader = LineReader::new();
    let mut buffer = String::new();

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
                Command::Info => {
                    rho.info();
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
                    rho.insert(&table, row).expect("Could not insert row");
                },
                Command::Drop { table } => {
                    rho.drop_table(&table).expect("Could not drop table");
                },
                Command::Select { table } => {
                    let values = rho.select(&table).expect("Could not select rows");
                    for value in values {
                        println!("{}", value);
                    }
                },
            }
        } else {
            // STATEMENT
            match rho.exec(&buffer) {
                Ok(_) => {
                    println!();
                    println!("ok.");
                },
                Err(e) => {
                    println!("{:?}", e);
                    println!("error.");
                }
            }
        }
    }

    // CLEANUP
    line_reader.close();
}
