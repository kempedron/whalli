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

fn main() {
    let source_code = "print 10 + 20 + 30;";
    println!("Код: {}", source_code);

    let mut lexer = Lexer::new(source_code);
    let tokens = lexer.tokenize();

    let mut parser = Parser::new(tokens);
    let ast = parser.parse();

    let compiler = Compiler::new();
    let bytecode = compiler.compile(&ast);

    println!("--- Выполнение ---");
    let mut vm = VM::new(bytecode);
    vm.run();
}