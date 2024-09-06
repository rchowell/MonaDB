use std::env;
use std::path::PathBuf;

use rho::{table::{Schema, Table}, value::JValue, Rho};
use rustyline::{error::ReadlineError, history::DefaultHistory, Config, DefaultEditor, EditMode, Editor};

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
    editor: Editor<(), DefaultHistory>,
}

impl LineReader {
    pub fn new() -> LineReader {
        let config = Config::builder()
            .edit_mode(EditMode::Vi)
            .build();
        let mut editor = DefaultEditor::with_config(config).unwrap();
        editor.load_history(".rho_history").unwrap();
        LineReader { editor }
    }

    pub fn close(&mut self) {
        self.editor.save_history(".rho_history").unwrap();
    }

    pub fn read_line(&mut self, buffer: &mut String, prompt: &str) -> Option<()> {
        let readline = self.editor.readline(prompt);
        match readline {
            Ok(line) => {
                self.editor.add_history_entry(line.as_str());
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

fn main() {

    // OPEN DATABASE 
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <file>", args[0]);
        std::process::exit(1);
    }
    let path: PathBuf = PathBuf::from(&args[1]);
    let mut rho = Rho::open(&path).expect("Could not open rho");

    // REPL 
    let mut line_reader = LineReader::new();
    let mut line = String::new();

    // MAIN LOOP
    loop {
        if line_reader.read_line(&mut line, ">> ") == None {
            // EOF or interrupt
            break;
        }
        if line.is_empty() {
            continue;
        }
        if line.starts_with(".") {
            // PARSE COMMAND
            let line = line.strip_prefix(".").unwrap();
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
                    let schema = Schema::empty();
                    let table = Table::new(table, schema);
                    rho.create_table(&table).expect("Could not create table");
                },
                Command::Insert { table, value } => {
                    let row = JValue::from_str(&value).expect("Could not parse row");
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
            match rho.exec(&line) {
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
