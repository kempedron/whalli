use whalli::compiler::Compiler;
use whalli::lexer::Lexer;
use whalli::parser::Parser;
use whalli::vm::VM;

pub fn run_code(code: &str) -> VM {
    let mut lexer = Lexer::new(code);
    let tokens = lexer.tokenize().expect("Lexer error");
    let mut parser = Parser::new(tokens);
    let ast = parser.parse().expect("Parser error");
    let compiler = Compiler::new();
    let bytecode = compiler.compile(&ast);
    let mut vm = VM::new(bytecode);
    vm.run().expect("Runtime error");
    vm
}
