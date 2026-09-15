mod ast;
mod compiler;
mod heap;
mod lexer;
mod opcode;
mod parser;
pub mod stdlib;
mod value;
mod vm;

use compiler::Compiler;
use lexer::Lexer;
use parser::Parser;
use std::env;
use std::fs;
use vm::VM;

fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let filename = args[1].as_str();
    let source_code = fs::read_to_string(filename)?;

    let mut lexer = Lexer::new(source_code.as_str());
    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        Err(err) => {
            println!("Traceback (most recent call last):");
            println!("  File \"{}\", line {}", filename, err.line);

            let lines: Vec<&str> = source_code.lines().collect();
            if err.line > 0 && err.line <= lines.len() {
                let code_line = lines[err.line - 1].trim();
                println!("    {}", code_line);
                println!("    \x1b[31m{}\x1b[0m", "^".repeat(code_line.len()));
            }

            println!("\x1b[31mLexError: {}\x1b[0m", err.message);
            std::process::exit(1);
        }
    };

    let mut parser = Parser::new(tokens);
    let ast = match parser.parse() {
        Ok(tree) => tree,
        Err(err) => {
            println!("Traceback (most recent call last):");
            println!("  File \"{}\", line {}", filename, err.line);

            let lines: Vec<&str> = source_code.lines().collect();
            if err.line > 0 && err.line <= lines.len() {
                let code_line = lines[err.line - 1].trim();
                println!("    {}", code_line);
                println!("    \x1b[31m{}\x1b[0m", "^".repeat(code_line.len()));
            }

            println!("\x1b[31mSyntaxError: {}\x1b[0m", err.message);
            std::process::exit(1);
        }
    };

    let compiler = Compiler::new();
    let bytecode = compiler.compile(&ast);

    let mut vm = VM::new(bytecode);

    if let Err(err) = vm.run() {
        println!("Traceback (most recent call last):");
        println!("  File \"{}\", line {}", filename, err.line);

        let lines: Vec<&str> = source_code.lines().collect();
        if err.line > 0 && err.line <= lines.len() {
            let code_line = lines[err.line - 1].trim();
            println!("    {}", code_line);
            println!("    \x1b[31m{}\x1b[0m", "^".repeat(code_line.len()));
        }
        println!("\x1b[31mRuntimeError: {}\x1b[0m", err.message);
    }

    Ok(())
}
