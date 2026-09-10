mod value;
mod opcode;
mod vm;
mod ast;
mod compiler;
mod lexer;
mod parser;

use lexer::Lexer;
use parser::Parser;
use compiler::Compiler;
use vm::VM;
use std::fs;
use std::env;

fn main() -> std::io::Result<()> {
    // let source_code = "print 10 + 20 + 30;";
    // whalli main.wh
    let args: Vec<String> = env::args().collect();
    let source_code = fs::read_to_string(args[1].as_str())?;
    println!("Код: {}", source_code);

    let mut lexer = Lexer::new(source_code.as_str());
    let tokens = lexer.tokenize();

    let mut parser = Parser::new(tokens);
    let ast = parser.parse();

    let compiler = Compiler::new();
    let bytecode = compiler.compile(&ast);

    println!("--- Выполнение ---");
    let mut vm = VM::new(bytecode);
    vm.run();
    Ok(())
}